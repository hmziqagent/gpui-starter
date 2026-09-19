//! Re-entry bridge: browser events → GPUI updates.
//!
//! DOM event callbacks (`hashchange`, `online`, …) fire on the browser's
//! event loop, outside any GPUI update — they cannot borrow the [`App`]
//! directly. This module installs a page-lifetime flume queue: DOM closures
//! enqueue `FnOnce(&mut App)` jobs ([`dispatch`]), and a single GPUI task
//! drains the queue inside `cx.update`. Same channel-then-update shape as
//! `desktop_actions::watch_config_dir_reactive`, just for browser events.

use std::sync::OnceLock;

use gpui::App;

const LOG: &str = "gpui_starter::web::dispatch";

static QUEUE: OnceLock<flume::Sender<Box<dyn FnOnce(&mut App) + Send>>> = OnceLock::new();

/// Spawn the drain task and stash the sender for later [`dispatch`] calls.
///
/// Called once from [`super::install`] (app init). The task runs for the
/// page lifetime: the sender is a page-lifetime static and the receiver is
/// only dropped when the whole app tears down.
pub fn install(cx: &mut App) {
    let (tx, rx) = flume::unbounded::<Box<dyn FnOnce(&mut App) + Send>>();
    let _ = QUEUE.set(tx);
    cx.spawn(async move |cx| {
        while let Ok(job) = rx.recv_async().await {
            // AsyncApp::update panics if the app was released, but this task
            // is owned by that same app — it cannot outlive it, so the panic
            // path is unreachable and there is no error to handle here.
            cx.update(|cx| job(cx));
        }
    })
    .detach();
}

/// Enqueue a closure to run with `&mut App` on the next queue drain.
///
/// Best-effort by design: before [`install`] or after the app is released
/// the job is dropped with a log line — a stale `hashchange` must never
/// crash the page.
pub fn dispatch(job: impl FnOnce(&mut App) + Send + 'static) {
    match QUEUE.get() {
        Some(tx) => {
            let _ = tx.send(Box::new(job));
        }
        None => {
            tracing::warn!(
                target: LOG,
                "browser event dispatched before web::install; dropping job"
            );
        }
    }
}
