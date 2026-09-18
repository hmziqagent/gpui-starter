//! Application entry points.
//!
//! [`bootstrap`] is the cfg-neutral startup path shared by the native binary
//! (`src/main.rs`) and the wasm entry ([`start`]): single-instance preflight,
//! GPUI application construction, the init/create-window closure, and the
//! post-run exec-reload tail (unix only). Keeping it in the lib means the
//! browser boots through the exact same code path as the desktop build —
//! only the entry function differs.
//!
//! On wasm, `#[wasm_bindgen(start)]` runs `start()` when the generated JS
//! glue instantiates the module. `gpui_platform::application()` already
//! dispatches to `gpui_web::WebPlatform` on wasm, so the same `run` closure
//! boots the app in a browser tab; platform services that do not exist on
//! the web (storage, keyring, tray, single-instance lock, …) degrade via
//! their wasm stubs and report unsupported capabilities.

use gpui::App;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Shared application bootstrap, invoked by the native `main` and the wasm
/// `start` entry alike.
pub fn bootstrap() {
    // Wasm: install the console panic hook + logging FIRST — before any
    // application code can fail — so errors surface in the browser console.
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();

    let preflight = crate::single_instance::preflight();
    if !preflight.should_start {
        return;
    }
    let startup_runtime = preflight.runtime;
    let startup_deep_link = preflight.initial_deep_link;

    // Native: default platform application.
    #[cfg(not(target_family = "wasm"))]
    let app_runtime =
        gpui_platform::application().with_assets(crate::app::assets::CombinedAssets::new());

    // Wasm: force the WebGL2 backend. The default `Auto` preference selects
    // WebGPU, whose device is unstable in software-rasterized/headless
    // environments (wgpu reports `device lost: external Instance reference
    // no longer exists` and rendering stops on a blank canvas); wgpu's
    // WebGL2 backend renders identically and is stable everywhere.
    #[cfg(target_family = "wasm")]
    let app_runtime =
        gpui_platform::application_with_web_backend(gpui_platform::WebBackendPreference::WebGl)
            .with_assets(crate::app::assets::CombinedAssets::new());
    let launch = move |cx: &mut App| {
        crate::app::init(cx);
        if let Some(runtime) = startup_runtime {
            crate::single_instance::install(runtime, cx);
        }
        if let Some(link) = startup_deep_link {
            crate::events::emit(crate::events::AppEventKind::DeepLinkReceived(link), cx);
        }

        #[cfg(target_os = "macos")]
        crate::tray::setup(cx);

        cx.activate(true);
        crate::app::create_new_window("My App", cx);
    };

    // Native: `Platform::run` blocks for the whole app lifetime (platform
    // event loop), so the stack frame keeps the App alive.
    #[cfg(not(target_family = "wasm"))]
    app_runtime.run(launch);

    // Wasm: `WebPlatform::run` invokes the launch closure and returns
    // immediately (the run loop belongs to the browser). `run()` would then
    // drop the last strong App reference before any spawned task can run
    // ("app was released before async operation completed"). `run_embedded`
    // returns an `ApplicationHandle` that owns the App; intentionally leak it
    // so the app lives for the lifetime of the page.
    #[cfg(target_family = "wasm")]
    std::mem::forget(app_runtime.run_embedded(launch));

    // After GPUI has fully shut down (the run closure returned), re-exec the
    // binary only when a restart was requested. exec_reload() never returns on
    // success; on failure it logs and we fall through to a normal exit.
    //
    // CAVEAT (needs runtime verification): the SingleInstanceRuntime is held
    // as a GPUI Global inside run(); for exec() to let the relaunched process
    // win preflight(), that lock must be released first. single-instance-0.3.3
    // binds via an abstract Unix socket with no SOCK_CLOEXEC, and exec() does
    // not run Rust dtors, so release depends on the Application (and its
    // globals) being dropped when run() returns. If a restart ever silently
    // no-ops, set FD_CLOEXEC on the lock fd (needs a crate accessor) or
    // spawn-then-exit instead of exec(). See reload.rs.
    //
    // Wasm is not unix and has no exec-reload; the tail is compiled out there.
    #[cfg(unix)]
    {
        #[allow(clippy::collapsible_if)]
        if crate::app::is_reload_requested() {
            if let Err(err) = crate::app::exec_reload() {
                eprintln!("reload failed: {err}");
            }
        }
    }
}

/// Wasm entry point — invoked by the `wasm-bindgen`-generated JS glue when
/// the module instantiates (`#[wasm_bindgen(start)]`).
#[cfg(target_family = "wasm")]
#[wasm_bindgen(start)]
pub fn start() {
    bootstrap();
}
