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

    let app_runtime =
        gpui_platform::application().with_assets(crate::app::assets::CombinedAssets::new());
    app_runtime.run(move |cx| {
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
    });

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
