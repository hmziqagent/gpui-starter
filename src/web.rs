//! Application entry points: one [`bootstrap`] shared by the native binary
//! and the wasm `start` entry, so both boot the same code path.

use gpui::App;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Shared application bootstrap, invoked by the native `main` and the wasm
/// `start` entry alike.
pub fn bootstrap() {
    // Wasm: console panic hook + logging first, so early failures surface.
    #[cfg(target_family = "wasm")]
    gpui_platform::web_init();

    let preflight = crate::single_instance::preflight();
    if !preflight.should_start {
        return;
    }
    let startup_runtime = preflight.runtime;
    let startup_deep_link = preflight.initial_deep_link;

    #[cfg(not(target_family = "wasm"))]
    let app_runtime =
        gpui_platform::application().with_assets(crate::app::assets::CombinedAssets::new());

    // Wasm: force WebGL2 — Auto/WebGPU dies with "device lost" on software
    // rasterizers; WebGL2 is stable everywhere and renders identically.
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

    // Native: `Platform::run` blocks for the whole app lifetime, so the stack
    // frame keeps the App alive.
    #[cfg(not(target_family = "wasm"))]
    app_runtime.run(launch);

    // Wasm: `run` returns immediately; leak the `run_embedded` handle so the
    // App lives for the page lifetime.
    #[cfg(target_family = "wasm")]
    std::mem::forget(app_runtime.run_embedded(launch));

    // Re-exec after shutdown when a restart was requested. A silent no-op
    // means the instance lock outlived run() (exec skips dtors).
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
