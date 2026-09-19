use super::*;

#[test]
fn lifecycle_stage_starts_as_starting() {
    let state = LifecycleState::default();
    assert_eq!(state.stage, LifecycleStage::Starting);
}

#[test]
fn render_panic_flag_roundtrip() {
    // The flag is process-global; consume any stale value first so the
    // assertions below are deterministic even with parallel tests.
    let _ = take_render_panic();
    RENDER_PANIC_OCCURRED.store(true, Ordering::SeqCst);
    assert!(take_render_panic(), "first read should be true");
    assert!(
        !take_render_panic(),
        "second read should be false (flag reset)"
    );
}

#[test]
fn enter_render_path_sets_and_clears() {
    assert!(!in_render_path());
    {
        let _guard = enter_render_path();
        assert!(in_render_path());
    }
    assert!(!in_render_path());
}

#[test]
fn track_recent_error_keeps_limit() {
    for i in 0..25 {
        track_recent_error(format!("err-{i}"));
    }
    let slot = RECENT_ERRORS.get().unwrap();
    let guard = slot.lock().unwrap();
    assert!(guard.len() <= 20, "ring buffer must cap at 20 entries");
    assert_eq!(guard.last().map(String::as_str), Some("err-24"));
}

#[test]
fn crash_marker_roundtrip_in_data_dir() {
    let marker = test_data_dir().join("crash-marker");
    let _ = std::fs::remove_file(&marker);

    write_crash_marker();
    let contents = check_previous_crash();
    assert!(contents.is_some(), "marker should exist after write");
    assert!(
        contents.unwrap().starts_with("pid="),
        "marker should start with pid="
    );

    remove_crash_marker();
    assert!(check_previous_crash().is_none(), "marker should be removed");
    assert!(!marker.exists());
}

/// Point the once-set data dir at a shared temp dir for the marker tests.
fn test_data_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("tempdir").keep();
        set_app_data_dir(dir.clone());
        dir
    })
}
