//! Wasm-only browser integration bridges.
//!
//! This module hosts the web-platform replacements behind the app's existing
//! service abstractions (house rule: no parallel web-only service layer):
//!
//! - [`clipboard`]: `navigator.clipboard.writeText` fire-and-forget writes,
//!   used by [`crate::platform::clipboard::copy`] and
//!   [`crate::services::desktop_actions`].
//! - [`router`]: hash deep links (`#/settings`, …) wired to the existing
//!   [`crate::routes::AppRoute`] registry through the History API.
//! - [`document_meta`]: the tray-equivalent — a canvas-drawn favicon with an
//!   unread badge + connectivity state, and a `document.title` mirror.
//! - [`connectivity`]: `navigator.onLine` + online/offline events replacing
//!   the "assume online" stub in [`crate::services::connectivity`].
//! - [`dispatch`]: the single re-entry bridge that lets DOM event callbacks
//!   (which fire outside GPUI's update loop) run closures with `&mut App`.
//!
//! Everything here is compiled only for `wasm32-unknown-unknown`; the native
//! builds never see it. [`install`] is called from [`crate::app::init`]'s wasm
//! wiring.

pub mod clipboard;
pub mod connectivity;
pub mod dispatch;
pub mod document_meta;
pub mod router;

use gpui::App;

/// Install every browser integration at app init.
///
/// Order matters only in that [`dispatch`] must come first: the router and
/// connectivity listeners push work through its queue. The heavy lifting runs
/// in the submodules; see their docs for the listener lifetime policy
/// (closures are leaked for the page lifetime — the house pattern shared with
/// the `ApplicationHandle` leak in `src/web.rs`).
pub fn install(cx: &mut App) {
    dispatch::install(cx);
    router::install();
    document_meta::install(cx);
    connectivity::install_listeners();
}
