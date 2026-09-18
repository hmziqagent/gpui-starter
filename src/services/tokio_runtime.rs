use std::sync::Arc;

use gpui::{App, Global};

/// Dedicated tokio runtime for I/O-bound work (HTTP, etc.).
///
/// GPUI's executor uses its own scheduler (GCD on macOS) and is **not** a tokio
/// runtime. Any code that depends on tokio — `reqwest`, `tokio::net`, `tokio::time`
/// — must run inside this runtime via [`Self::spawn`].
pub struct TokioRuntime {
    pub runtime: Arc<tokio::runtime::Runtime>,
    pub http_client: reqwest::Client,
}

#[cfg(not(target_family = "wasm"))]
impl TokioRuntime {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("gpui-io")
            .build()
            .expect("failed to create tokio runtime");
        tracing::info!(target: "gpui_starter::tokio_runtime", "tokio runtime created");

        // Build the reqwest client inside the runtime context so the connector
        // can resolve DNS via tokio.
        let _guard = runtime.enter();
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build reqwest client");

        Self {
            runtime: Arc::new(runtime),
            http_client,
        }
    }

    /// Spawn an async task on the tokio runtime.
    pub fn spawn<F>(&self, f: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(f)
    }
}

/// Wasm: tokio has no multi-thread runtime and `reqwest`'s wasm
/// `ClientBuilder` supports neither `timeout`, `cookie_store` nor `redirect`
/// policies — build the plain client and a `new_current_thread` runtime shim.
///
/// NOTE: nothing drives this runtime on wasm (there is no blocking
/// `block_on`), so futures spawned directly on it will never complete.
/// Async work on wasm must instead run on GPUI's own executor
/// (`cx.spawn` / `cx.background_executor()`), which drives futures on the
/// browser frame loop; the reqwest wasm client works under any executor.
#[cfg(target_family = "wasm")]
impl TokioRuntime {
    pub fn new() -> Self {
        // No drivers on wasm: this shim is never driven (see the impl note
        // below), and `enable_all()` constructs the tokio time wheel, whose
        // setup reads the clock — `tokio::time::Instant` IS
        // `std::time::Instant`, which panics at runtime on
        // wasm32-unknown-unknown ("time not implemented on this platform").
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("failed to create tokio runtime shim");
        tracing::info!(
            target: "gpui_starter::tokio_runtime",
            "tokio current-thread runtime shim created (wasm; not driven)"
        );

        let http_client = reqwest::Client::builder()
            .build()
            .expect("failed to build reqwest client");

        Self {
            runtime: Arc::new(runtime),
            http_client,
        }
    }

    /// Spawn an async task on the tokio runtime. On wasm this runtime is not
    /// driven (see the impl note above) — prefer GPUI's executor instead.
    pub fn spawn<F>(&self, f: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(f)
    }
}

impl Default for TokioRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// GPUI Global that holds the shared tokio runtime.
pub struct TokioRuntimeGlobal(pub TokioRuntime);

impl Global for TokioRuntimeGlobal {}

/// Borrow the shared tokio runtime handle, if a [`TokioRuntimeGlobal`] has been
/// installed. Returns `None` when the runtime is absent (callers should
/// degrade gracefully — e.g. log + return an early result).
pub fn handle(cx: &App) -> Option<Arc<tokio::runtime::Runtime>> {
    cx.try_global::<TokioRuntimeGlobal>()
        .map(|g| g.0.runtime.clone())
}

/// Borrow the shared tokio runtime handle **and** HTTP client together, if a
/// [`TokioRuntimeGlobal`] has been installed. Returns `None` when the runtime
/// is absent (callers should degrade gracefully — e.g. log + return an early
/// result). Prefer this over back-to-back
/// `.global::<TokioRuntimeGlobal>().0.runtime.clone()` +
/// `.0.http_client.clone()` at call sites that need both halves of the duo;
/// it halves the global lookups and keeps the pair in sync.
pub fn runtime_and_client(cx: &App) -> Option<(Arc<tokio::runtime::Runtime>, reqwest::Client)> {
    cx.try_global::<TokioRuntimeGlobal>()
        .map(|g| (g.0.runtime.clone(), g.0.http_client.clone()))
}

/// Spawn `future` on the shared tokio runtime, returning the `JoinHandle`.
///
/// Returns `None` when no runtime is installed (graceful degrade). Prefer this
/// over `TokioRuntimeGlobal::0.runtime.spawn(...)` at call sites that already
/// hold an `&App` reference.
pub fn spawn<F>(cx: &App, future: F) -> Option<tokio::task::JoinHandle<F::Output>>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle(cx).map(|runtime| runtime.spawn(future))
}
