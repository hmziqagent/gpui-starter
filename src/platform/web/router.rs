//! Hash deep links: `location.hash` ↔ [`AppRoute`], with History API sync.
//!
//! The route registry itself lives in [`crate::shell::route`]; this module is
//! the wasm-only browser wiring around it:
//!
//! - **Boot**: an explicit `#/…` hash is parsed and emitted as a
//!   [`AppEventKind::Navigate`] event (the same queue the native deep-link
//!   path uses, drained by `AppRoot`), and the boot URL is canonicalized with
//!   `replaceState` so it is bookmarkable without spawning a history entry.
//!   An *empty* hash is not a deep link — the persisted startup route wins.
//! - **Back/forward**: `hashchange` + `popstate` listeners translate external
//!   URL movement into Navigate events.
//! - **In-app navigation**: [`push_route_hash`] (called from
//!   `AppRoot::set_route`'s wasm arm) pushes a history entry so the URL bar
//!   tracks in-app navigation and Back steps through it. It no-ops when the
//!   hash already matches — that is the external-navigation case, where the
//!   browser already moved history and re-pushing would double every entry.
//!
//! Listener closures are leaked for the page lifetime (house pattern, same as
//! the `ApplicationHandle` leak in `src/web.rs`).

use wasm_bindgen::{JsCast as _, JsValue, prelude::Closure};

use crate::{events::AppEventKind, routes::AppRoute};

const LOG: &str = "gpui_starter::web::router";

/// Install hash routing at app init (see module docs for the flow).
pub fn install() {
    let hash = current_hash();
    if hash.is_empty() || hash == "#" {
        // No deep link at boot: keep the persisted/configured startup route.
        tracing::debug!(
            target: LOG,
            "no location.hash at boot; keeping configured route"
        );
    } else {
        match AppRoute::from_hash(&hash) {
            Ok(route) => {
                replace_history(&route);
                tracing::info!(
                    target: LOG,
                    hash = %hash,
                    route = ?route,
                    "booting from hash deep link"
                );
                // Apply through the dispatch queue, NOT inline: install runs
                // during app::init, possibly before the window/AppRoot (and
                // its AppEventQueue observer) exist, and observe_global never
                // replays queued events at registration. The job is correct
                // under EITHER timing:
                //   - window not yet created: the config write below is what
                //     AppRoot::new reads for its initial active_route;
                //   - window already created: the Navigate event is drained
                //     by AppRoot's observer (default_global always pushes a
                //     NotifyGlobalObservers effect, so emit reaches it), and
                //     the latent event becomes a same-route no-op.
                super::dispatch::dispatch(move |cx| {
                    crate::app_state::update_config(cx, |config| {
                        config.active_route = route.clone();
                    });
                    crate::events::emit(AppEventKind::Navigate(route), cx);
                });
            }
            Err(err) => {
                tracing::warn!(
                    target: LOG,
                    hash = %hash,
                    error = %err,
                    "invalid location.hash at boot; keeping configured route"
                );
            }
        }
    }
    install_listeners();
}

/// Keep `location.hash` in sync after in-app navigation (wasm arm of
/// `AppRoot::set_route`). Pushes a history entry only when the hash actually
/// differs — external navigation (`hashchange`/`popstate`) has already moved
/// history by the time the resulting Navigate event reaches `set_route`, and
/// re-pushing there would grow the history stack on every Back press.
pub fn push_route_hash(route: &AppRoute) {
    let hash = route.to_hash();
    if hash == current_hash() {
        return;
    }
    let url = current_url_with_hash(&hash);
    match history() {
        Ok(history) => {
            if let Err(err) = history.push_state_with_url(&JsValue::NULL, "", Some(&url)) {
                tracing::warn!(
                    target: LOG,
                    error = ?err,
                    hash = %hash,
                    "history.pushState failed"
                );
            }
        }
        Err(err) => tracing::warn!(target: LOG, error = ?err, "history API unavailable"),
    }
}

fn install_listeners() {
    let Some(window) = web_sys::window() else {
        tracing::warn!(target: LOG, "no window; hash routing listeners not installed");
        return;
    };
    let target: &web_sys::EventTarget = window.as_ref();
    for event in ["hashchange", "popstate"] {
        let handler: Closure<dyn FnMut(JsValue)> =
            Closure::new(|_event: JsValue| on_external_navigation());
        let registered = target
            .add_event_listener_with_callback(event, handler.as_ref().unchecked_ref())
            .is_ok();
        if registered {
            // Leaked intentionally: the listener must live for the page
            // lifetime (house pattern; see the module docs).
            handler.forget();
        } else {
            tracing::warn!(target: LOG, event, "could not install hash routing listener");
        }
    }
}

/// External URL movement (back/forward/typed hash): parse the new hash and
/// emit a Navigate event through the dispatch queue.
fn on_external_navigation() {
    let hash = current_hash();
    match AppRoute::from_hash(&hash) {
        Ok(route) => {
            tracing::info!(
                target: LOG,
                hash = %hash,
                route = ?route,
                "external hash navigation"
            );
            super::dispatch::dispatch(move |cx| {
                crate::events::emit(AppEventKind::Navigate(route), cx);
            });
        }
        Err(err) => {
            tracing::warn!(
                target: LOG,
                hash = %hash,
                error = %err,
                "ignoring invalid location.hash change"
            );
        }
    }
}

fn replace_history(route: &AppRoute) {
    let url = current_url_with_hash(&route.to_hash());
    match history() {
        Ok(history) => {
            if let Err(err) = history.replace_state_with_url(&JsValue::NULL, "", Some(&url)) {
                tracing::warn!(
                    target: LOG,
                    error = ?err,
                    "history.replaceState failed (boot URL not canonicalized)"
                );
            }
        }
        Err(err) => tracing::warn!(target: LOG, error = ?err, "history API unavailable"),
    }
}

fn history() -> Result<web_sys::History, JsValue> {
    web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))
        .and_then(|window| window.history())
}

/// Current `location.hash` including the leading `#` (empty when absent).
fn current_hash() -> String {
    web_sys::window()
        .and_then(|window| {
            window
                .location()
                .hash()
                .ok()
                .map(|hash| hash.trim().to_string())
        })
        .unwrap_or_default()
}

/// Absolute-path URL (pathname + search + new hash) for push/replaceState.
fn current_url_with_hash(hash: &str) -> String {
    let Some(window) = web_sys::window() else {
        return hash.to_string();
    };
    let location = window.location();
    let pathname = location.pathname().unwrap_or_else(|_| "/".into());
    let search = location.search().unwrap_or_default();
    format!("{pathname}{search}{hash}")
}
