//! Wasm clipboard bridge: `navigator.clipboard.writeText`, fire-and-forget.
//!
//! The app's clipboard wrappers ([`crate::platform::clipboard::copy::set_text`],
//! [`crate::services::desktop_actions::copy_text`]) are synchronous
//! `-> Result` functions, but the web clipboard API is Promise-only. The
//! accepted contract (routing finding): dispatch the write, log a Promise
//! rejection through tracing, return `Ok` immediately. Image writes stay an
//! explicit error — `navigator.clipboard.write(ClipboardItem)` is
//! PNG+secure-context-only and has no raw-RGB path (parity with the
//! arboard-less stub it replaces).

use js_sys::Promise;
use wasm_bindgen::{JsValue, prelude::Closure};

const LOG: &str = "gpui_starter::web::clipboard";

/// Whether `navigator.clipboard` exists (secure contexts only).
///
/// Both harness origins (127.0.0.1 and the HTTPS tunnel) are secure, so this
/// is `true` in practice; it keeps the capability snapshot honest when the
/// page is served from an insecure origin.
pub fn is_available() -> bool {
    clipboard_handle().is_some()
}

/// Write text through the async Clipboard API; see the module docs for the
/// fire-and-forget contract.
pub fn write_text_fire_and_forget(text: &str) {
    let Some(clipboard) = clipboard_handle() else {
        tracing::warn!(
            target: LOG,
            "navigator.clipboard unavailable (insecure context?); text write dropped"
        );
        return;
    };
    log_rejection(&clipboard.write_text(text), "clipboard.writeText");
}

/// `navigator.clipboard`, checked for existence first.
///
/// web-sys types the getter as non-nullable, but browsers leave the property
/// `undefined` in insecure contexts — probing with `Reflect::has` avoids
/// calling into an undefined object.
fn clipboard_handle() -> Option<web_sys::Clipboard> {
    let navigator = web_sys::window()?.navigator();
    let has_clipboard = js_sys::Reflect::has(
        &JsValue::from(navigator.clone()),
        &JsValue::from_str("clipboard"),
    )
    .unwrap_or(false);
    if has_clipboard {
        Some(navigator.clipboard())
    } else {
        None
    }
}

/// Attach a rejection logger to a fire-and-forget promise.
///
/// Success needs no handler. The catch closure fires at most once and is
/// intentionally leaked — it must outlive this stack frame for as long as the
/// Promise is pending, and there is nothing to reclaim afterwards (the house
/// leak-intentionally pattern for page-lifetime JS handles).
fn log_rejection(promise: &Promise, what: &'static str) {
    let on_reject: Closure<dyn FnMut(JsValue)> = Closure::new(move |err: JsValue| {
        tracing::warn!(
            target: LOG,
            what,
            error = ?err,
            "browser promise rejected"
        );
    });
    let _ = promise.catch(&on_reject);
    on_reject.forget();
}
