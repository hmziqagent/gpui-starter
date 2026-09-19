#[cfg(not(target_family = "wasm"))]
mod notify_rust;
#[cfg(feature = "notifications-portal")]
mod portal;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod user_notify;
#[cfg(target_family = "wasm")]
mod web;

#[cfg(not(target_family = "wasm"))]
pub use notify_rust::NotifyRustBackend;
#[cfg(feature = "notifications-portal")]
pub use portal::PortalBackend;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use user_notify::UserNotifyBackend;
#[cfg(target_family = "wasm")]
pub use web::WebNotificationBackend;

use async_trait::async_trait;

use crate::notifications::{
    NotificationBackendKind, NotificationCapabilities, NotificationPermissionState,
    NotificationRequest,
};

// async_trait's generated wrapper fns carry #[must_use] and return boxed
// futures (already #[must_use]) — new nightly clippy flags the generated code
// as double_must_use. The attribute is macro-emitted, so the allow lives here
// at the macro use site.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait NotificationBackend: Send + Sync {
    fn kind(&self) -> NotificationBackendKind;
    fn capabilities(&self) -> NotificationCapabilities;
    async fn refresh_permission_state(&self) -> NotificationPermissionState;
    async fn request_permission(&self) -> NotificationPermissionState;
    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()>;
}

// --- Web Notifications mapping helpers (target-neutral) ------------------------
//
// Pure functions so the Web-permission mapping is unit-testable on native
// builds; only the wasm-only `WebNotificationBackend` calls them at runtime.
// They live HERE (not in `service::types`) because `service::types` is
// private to the `service` subtree — this backend module cannot path to it,
// and the fixed re-export lists are outside this module's control.

/// Map a Web Notifications permission string — `Notification.permission` or
/// the `requestPermission()` resolution value — onto
/// [`NotificationPermissionState`].
///
/// The spec enum is exactly `"granted" | "denied" | "default"`; any other
/// value (a future spec addition, junk, wrong casing, the Permissions API's
/// `"prompt"`) maps to `Unknown` instead of guessing.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub(crate) fn web_permission_to_state(permission: &str) -> NotificationPermissionState {
    match permission {
        "granted" => NotificationPermissionState::Authorized,
        "denied" => NotificationPermissionState::Denied,
        "default" => NotificationPermissionState::NotDetermined,
        _ => NotificationPermissionState::Unknown,
    }
}

/// `NotificationOptions.tag` for a request: `thread_id` doubles as the web
/// tag, so same-thread notifications replace each other in the OS tray
/// (matching the per-thread coalescing the desktop backends get from the
/// daemon). `None` leaves the tag unset — every notification stacks.
#[cfg_attr(not(target_family = "wasm"), allow(dead_code))]
pub(crate) fn web_tag_for_request(request: &NotificationRequest) -> Option<String> {
    request.thread_id.clone()
}

/// Wasm: there is no OS notification daemon. With the Web Notifications API
/// taking the primary slot, this no-op backend keeps the secondary slot so
/// the service keeps its primary/secondary shape — when the web backend is
/// missing (very old browsers) or unauthorized, every send fails through it
/// and the notification service falls back to the in-app toast + inbox
/// policy (pure gpui, works in the browser).
#[cfg(target_family = "wasm")]
pub struct WasmStubBackend;

#[cfg(target_family = "wasm")]
impl WasmStubBackend {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_family = "wasm")]
impl Default for WasmStubBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_family = "wasm")]
#[async_trait]
impl NotificationBackend for WasmStubBackend {
    fn kind(&self) -> NotificationBackendKind {
        NotificationBackendKind::UiOnly
    }

    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            can_request_permission: false,
            can_read_permission_state: false,
            can_send_immediate_native: false,
            can_send_interactive: false,
            requires_packaged_runtime: false,
        }
    }

    async fn refresh_permission_state(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn request_permission(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()> {
        anyhow::bail!(
            "native notifications unavailable on wasm (title: {})",
            request.title
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_permission_strings_map_to_states() {
        assert_eq!(
            web_permission_to_state("granted"),
            NotificationPermissionState::Authorized
        );
        assert_eq!(
            web_permission_to_state("denied"),
            NotificationPermissionState::Denied
        );
        assert_eq!(
            web_permission_to_state("default"),
            NotificationPermissionState::NotDetermined
        );
    }

    #[test]
    fn web_permission_unknown_strings_map_to_unknown() {
        // The spec enum is lowercase-only; anything else (new spec value,
        // junk, wrong casing) must not masquerade as a real state.
        for value in ["", "Granted", "GRANTED", "prompt", "unsupported"] {
            assert_eq!(
                web_permission_to_state(value),
                NotificationPermissionState::Unknown,
                "\"{value}\" must map to Unknown"
            );
        }
    }

    #[test]
    fn web_tag_follows_thread_id() {
        // Thread-scoped requests coalesce under their thread id on the web
        // (same-tag notifications replace each other).
        let reply = NotificationRequest::reply("title", "body");
        assert_eq!(
            web_tag_for_request(&reply).as_deref(),
            Some("settings-reply")
        );

        // Unthreaded requests leave the tag unset so notifications stack.
        let foreground = NotificationRequest::foreground("title", "body");
        assert_eq!(web_tag_for_request(&foreground), None);
    }
}
