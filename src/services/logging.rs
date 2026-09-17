use gpui::{App, BorrowAppContext as _, Global};
#[cfg(not(target_family = "wasm"))]
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Clone, Debug)]
pub struct LoggingRuntime {
    pub enabled: bool,
    pub log_dir: String,
    pub file_prefix: String,
    pub has_guard: bool,
    pub last_error: Option<String>,
}

pub struct LoggingState {
    pub runtime: LoggingRuntime,
    #[cfg(not(target_family = "wasm"))]
    #[allow(dead_code)]
    guard: Option<WorkerGuard>,
}

impl Global for LoggingState {}

/// Native: rolling daily file appender + stderr layer.
#[cfg(not(target_family = "wasm"))]
pub fn initialize(cx: &mut App) {
    let paths = crate::app_state::paths(cx);
    let log_dir = paths.log_dir.display().to_string();
    let file_prefix = "gpui-starter.log".to_string();

    let file_appender = tracing_appender::rolling::daily(&paths.log_dir, &file_prefix);
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let init_result = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer),
        )
        .with(env_filter())
        .try_init();

    initialize_state(cx, log_dir, file_prefix, init_result, Some(guard));
}

/// Wasm: stderr-only layer (a no-op sink on wasm32-unknown-unknown — no
/// filesystem appender exists; browser console logging comes from the web
/// platform init's panic hook instead).
#[cfg(target_family = "wasm")]
pub fn initialize(cx: &mut App) {
    let paths = crate::app_state::paths(cx);
    let log_dir = paths.log_dir.display().to_string();
    let file_prefix = "gpui-starter.log".to_string();

    let init_result = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(env_filter())
        .try_init();

    initialize_state(cx, log_dir, file_prefix, init_result, ());
}

fn env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(
            "gpui::window=off"
                .parse()
                .expect("hardcoded directive is valid"),
        )
        .add_directive(
            format!("{}=trace", env!("CARGO_PKG_NAME"))
                .parse()
                .expect("hardcoded directive is valid"),
        )
        .add_directive(
            "gpui_starter=trace"
                .parse()
                .expect("hardcoded directive is valid"),
        )
        .add_directive(
            "user_notify=debug"
                .parse()
                .expect("hardcoded directive is valid"),
        )
        .add_directive(
            "notify_rust=debug"
                .parse()
                .expect("hardcoded directive is valid"),
        )
}

fn initialize_state(
    cx: &mut App,
    log_dir: String,
    file_prefix: String,
    init_result: Result<(), tracing_subscriber::util::TryInitError>,
    #[cfg(not(target_family = "wasm"))] guard: Option<WorkerGuard>,
    #[cfg(target_family = "wasm")] _guard: (),
) {
    // On wasm the guard parameter is a `()` placeholder (no file appender).
    #[cfg(target_family = "wasm")]
    let () = _guard;

    let runtime = match init_result {
        Ok(()) => {
            tracing::info!(
                target: "gpui_starter::logging",
                log_dir = %log_dir,
                file_prefix = %file_prefix,
                "logging initialized"
            );
            LoggingRuntime {
                enabled: true,
                log_dir,
                file_prefix,
                // No file appender guard exists on wasm.
                has_guard: cfg!(not(target_family = "wasm")),
                last_error: None,
            }
        }
        Err(err) => LoggingRuntime {
            enabled: false,
            log_dir,
            file_prefix,
            has_guard: cfg!(not(target_family = "wasm")),
            last_error: Some(err.to_string()),
        },
    };

    crate::capabilities::set(
        "file_logging",
        crate::capabilities::CapabilityStatus {
            // No rolling file appender on wasm — console/devtools only.
            supported: cfg!(not(target_family = "wasm")),
            enabled: runtime.enabled,
            degraded: runtime.last_error.is_some(),
            reason: runtime
                .last_error
                .as_ref()
                .map(|error| format!("logging init failed: {error}").into()),
            last_error: runtime.last_error.clone().map(Into::into),
        },
        cx,
    );

    cx.set_global(LoggingState {
        runtime,
        #[cfg(not(target_family = "wasm"))]
        guard,
    });
}

pub fn snapshot(cx: &App) -> LoggingRuntime {
    cx.try_global::<LoggingState>()
        .map(|state| state.runtime.clone())
        .unwrap_or(LoggingRuntime {
            enabled: false,
            log_dir: String::new(),
            file_prefix: "gpui-starter.log".to_string(),
            has_guard: false,
            last_error: Some("logging not initialized".to_string()),
        })
}

pub fn shutdown(cx: &mut App) {
    if let Some(runtime) = cx
        .try_global::<LoggingState>()
        .map(|state| state.runtime.clone())
    {
        tracing::debug!(
            target: "gpui_starter::logging",
            enabled = runtime.enabled,
            "logging shutdown requested"
        );
        cx.update_global::<LoggingState, _>(|state, _cx| {
            state.runtime.has_guard = false;
            #[cfg(not(target_family = "wasm"))]
            {
                state.guard = None;
            }
        });
    }
}
