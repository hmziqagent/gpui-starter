//! Wasm notification backend: the browser Web Notifications API behind the
//! shared [`NotificationBackend`] trait. Browsers render `new Notification(...)`
//! as a real OS notification; the installed-PWA push story is out of scope
//! (there is no server).
//!
//! Degrade paths, all explicit: browsers without `window.Notification` fail in
//! [`new`]; ungranted permission fails fast in `send` (avoiding the
//! constructor's bare `TypeError: Illegal constructor`); a rejected
//! `requestPermission` promise maps to [`NotificationPermissionState::Unavailable`].
//!
//! Send-future note: `#[async_trait]` futures must be `Send`, so no `JsValue`
//! may be held across an await. All JS interop stays inside synchronous
//! helpers and crosses the await boundary only through a `flume` channel of
//! plain `String`s.

use async_trait::async_trait;
use js_sys::Reflect;
use wasm_bindgen::{JsValue, prelude::Closure};

use super::{NotificationBackend, web_permission_to_state, web_tag_for_request};
use crate::notifications::{
    NotificationBackendKind, NotificationCapabilities, NotificationPermissionState,
    NotificationRequest,
};

const LOG: &str = "gpui_starter::notifications::web";

/// Notification icon shown in the OS banner. Served by the wasm harness
/// (PWA icon); if the file is missing the browser silently falls back to
/// its default icon — never an error.
const NOTIFICATION_ICON_URL: &str = "/icons/icon-192.png";

pub struct WebNotificationBackend;

impl WebNotificationBackend {
    /// Construct after probing that the browser actually exposes the API.
    ///
    /// `Err(reason)` is recorded by the service as its degrade reason — very
    /// old browsers land there and fall back to the stub secondary.
    pub fn new() -> Result<Self, String> {
        if notification_constructor().is_some() {
            tracing::info!(target: LOG, "Web Notification API available");
            Ok(Self)
        } else {
            let reason =
                "Web Notification API unavailable (window.Notification is undefined)".to_string();
            tracing::warn!(target: LOG, %reason, "Web Notification API missing");
            Err(reason)
        }
    }
}

#[async_trait]
impl NotificationBackend for WebNotificationBackend {
    fn kind(&self) -> NotificationBackendKind {
        NotificationBackendKind::Web
    }

    fn capabilities(&self) -> NotificationCapabilities {
        // The Web API surface: permission is readable AND requestable, and a
        // granted `new Notification()` is shown by the OS. Interactive
        // buttons (`actions`) are Chrome-desktop-only and real background
        // push needs a service worker + server — neither is advertised.
        NotificationCapabilities {
            can_request_permission: true,
            can_read_permission_state: true,
            can_send_immediate_native: true,
            can_send_interactive: false,
            requires_packaged_runtime: false,
        }
    }

    async fn refresh_permission_state(&self) -> NotificationPermissionState {
        match permission_string() {
            Some(permission) => web_permission_to_state(&permission),
            None => NotificationPermissionState::Unavailable(
                "Web Notification API unavailable".to_string(),
            ),
        }
    }

    async fn request_permission(&self) -> NotificationPermissionState {
        let Some(receiver) = spawn_permission_request() else {
            return NotificationPermissionState::Unavailable(
                "Web Notification API unavailable".to_string(),
            );
        };
        match receiver.recv_async().await {
            Ok(Ok(permission)) => web_permission_to_state(&permission),
            Ok(Err(err)) => {
                NotificationPermissionState::Unavailable(format!("requestPermission: {err}"))
            }
            // Unreachable in practice: the leaked closures keep a sender
            // alive for the page lifetime, so the channel never disconnects.
            // Mapped defensively instead of panicking on a flume quirk.
            Err(_) => NotificationPermissionState::Unknown,
        }
    }

    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()> {
        if notification_constructor().is_none() {
            anyhow::bail!("Web Notification API unavailable (window.Notification undefined)");
        }
        // Fail fast with a readable reason when unauthorized — constructing a
        // Notification without permission throws a bare TypeError whose
        // message ("Illegal constructor") explains nothing to the user.
        let permission = permission_string()
            .ok_or_else(|| anyhow::anyhow!("web notification permission could not be read"))?;
        if permission != "granted" {
            let state = web_permission_to_state(&permission);
            anyhow::bail!(
                "web notification permission is {} — request it from Settings → Notifications",
                state.label()
            );
        }

        let options = web_sys::NotificationOptions::new();
        web_sys::NotificationOptions::set_body(&options, &request.body);
        web_sys::NotificationOptions::set_icon(&options, NOTIFICATION_ICON_URL);
        if let Some(tag) = web_tag_for_request(request) {
            web_sys::NotificationOptions::set_tag(&options, &tag);
        }
        // Browsers play their default sound unless `silent` is set — map the
        // request's explicit sound opt-out onto it (forcing a custom sound is
        // not possible on the web).
        if !request.play_sound {
            web_sys::NotificationOptions::set_silent(&options, Some(true));
        }

        match web_sys::Notification::new_with_options(&request.title, &options) {
            // The Rust handle may drop freely: dropping is not `close()` —
            // the banner stays up until the user dismisses it.
            Ok(_notification) => {
                tracing::info!(target: LOG, "web notification shown");
                Ok(())
            }
            Err(err) => {
                let message = js_value_message(&err);
                tracing::warn!(target: LOG, error = %message, "new Notification() failed");
                anyhow::bail!("web notification rejected: {message}");
            }
        }
    }
}

/// The global `Notification` constructor, or `None` on browsers without the
/// API (also covers a missing `window`, e.g. non-browser embeddings).
///
/// `Reflect::get` on the window keeps this a pure existence probe — calling
/// any web-sys `Notification` static would throw a `ReferenceError` through
/// the generated shim on such browsers.
fn notification_constructor() -> Option<JsValue> {
    let window = web_sys::window()?;
    let constructor =
        Reflect::get(&JsValue::from(window), &JsValue::from_str("Notification")).ok()?;
    if constructor.is_undefined() || constructor.is_null() {
        None
    } else {
        Some(constructor)
    }
}

/// Read `Notification.permission` (`"granted" | "denied" | "default"`).
///
/// web-sys's typed `Notification::permission()` is gated behind a feature not
/// in this crate's web-sys union; `Reflect::get` reads the same property with
/// zero extra features.
fn permission_string() -> Option<String> {
    let constructor = notification_constructor()?;
    Reflect::get(&constructor, &JsValue::from_str("permission"))
        .ok()
        .and_then(|value| value.as_string())
}

/// Kick off `Notification.requestPermission()` and return the channel its
/// resolution lands on; `None` when the API is unavailable or the call threw.
///
/// The Promise is bridged with two `Closure`s + `then2` (wasm-bindgen-futures
/// is not a dependency); all JS handles stay inside this synchronous helper so
/// the awaiting future only holds the `Send` `flume::Receiver`. The closures
/// are leaked (house pattern) so they outlive the frame; the leaked senders
/// also keep the channel alive so the receiver never sees a disconnect.
fn spawn_permission_request() -> Option<flume::Receiver<Result<String, String>>> {
    notification_constructor()?;
    let promise = match web_sys::Notification::request_permission() {
        Ok(promise) => promise,
        Err(err) => {
            tracing::warn!(
                target: LOG,
                error = %js_value_message(&err),
                "requestPermission() call failed"
            );
            return None;
        }
    };

    let (tx, rx) = flume::unbounded::<Result<String, String>>();
    let tx_resolve = tx.clone();
    let on_resolve: Closure<dyn FnMut(JsValue)> = Closure::new(move |value: JsValue| {
        let _ = tx_resolve.send(Ok(value.as_string().unwrap_or_default()));
    });
    let on_reject: Closure<dyn FnMut(JsValue)> = Closure::new(move |err: JsValue| {
        let _ = tx.send(Err(js_value_message(&err)));
    });
    let _ = promise.then2(&on_resolve, &on_reject);
    on_resolve.forget();
    on_reject.forget();
    Some(rx)
}

/// Best-effort human-readable message from a JS exception value.
fn js_value_message(value: &JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
