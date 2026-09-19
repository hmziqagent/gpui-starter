//! Self-reload (exec) support for Unix: a flag set via [`request_reload`],
//! polled post-shutdown to re-exec the same binary with the original argv.

#![cfg(unix)]

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use tracing::{error, info};

const LOG: &str = "gpui_starter::reload";

/// Set by [`request_reload`], polled by the host after GPUI has shut down.
static RELOAD_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request an in-place relaunch after shutdown; call [`exec_reload`] only
/// once GPUI has fully torn down.
pub fn request_reload() {
    RELOAD_REQUESTED.store(true, Ordering::SeqCst);
    info!(target: LOG, "reload requested");
}

/// Returns `true` if [`request_reload`] has been called since the process
/// started (or since the flag was last cleared).
pub fn is_reload_requested() -> bool {
    RELOAD_REQUESTED.load(Ordering::SeqCst)
}

/// Re-exec with the original argv after GPUI shutdown; never returns on
/// success, returns `Err` without panicking otherwise.
pub fn exec_reload() -> Result<()> {
    use std::os::unix::process::CommandExt;

    info!(target: LOG, "executing in-place reload");

    let exe = std::env::current_exe().context("failed to resolve current executable path")?;

    // Re-launch with the original argv. Skip argv[0] (the program name) since
    // Command::new already supplies argv[0] from `exe`.
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(std::env::args().skip(1));

    let err = cmd.exec();

    // exec() returns the io::Error only when it fails — success never returns.
    error!(target: LOG, error = %err, "exec failed");
    Err(anyhow::anyhow!("failed to exec reload: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_roundtrip() {
        // Reset to a known state; tests run concurrently on the same static,
        // but this process does not actually exec so a benign toggle is fine.
        RELOAD_REQUESTED.store(false, Ordering::SeqCst);
        assert!(!is_reload_requested());
        request_reload();
        assert!(is_reload_requested());
        RELOAD_REQUESTED.store(false, Ordering::SeqCst);
    }
}
