use tempfile::tempdir;

use super::*;
use crate::sidebar::Page;

#[test]
fn save_and_load_config_uses_json_state_file() {
    let dir = tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    let config = AppConfig {
        active_route: AppRoute::page(Page::Settings),
        sidebar_collapsed: true,
        ..AppConfig::default()
    };

    let bytes = serde_json::to_vec(&config).unwrap();
    save_config(&state_file, &bytes).unwrap();
    let (loaded, err) = load_config(&state_file);

    assert_eq!(err, None);
    assert_eq!(loaded.active_route, AppRoute::page(Page::Settings));
    assert!(loaded.sidebar_collapsed);
}

#[test]
fn corrupt_config_is_quarantined() {
    let dir = tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    std::fs::write(&state_file, "{not-json").unwrap();

    let (loaded, err) = load_config(&state_file);

    assert_eq!(loaded, AppConfig::default());
    assert!(err.is_some());
    assert!(!state_file.exists());
    assert!(state_file.with_extension("json.bad").exists());
}

#[test]
fn oversized_config_is_quarantined_without_parsing() {
    let dir = tempdir().unwrap();
    let state_file = dir.path().join("state.json");
    std::fs::write(&state_file, vec![b' '; (MAX_STATE_FILE_BYTES + 1) as usize]).unwrap();

    let (loaded, err) = load_config(&state_file);

    assert_eq!(loaded, AppConfig::default());
    assert!(
        err.is_some(),
        "over-cap file must be reported as a load error"
    );
    assert!(!state_file.exists());
    assert!(state_file.with_extension("json.bad").exists());
}

#[test]
fn normalized_drops_implausible_window_bounds() {
    let cases = [
        PersistedWindowBounds {
            x: 0.0,
            y: 0.0,
            width: f32::NAN,
            height: 600.0,
        },
        PersistedWindowBounds {
            x: 0.0,
            y: 0.0,
            width: f32::INFINITY,
            height: 600.0,
        },
        PersistedWindowBounds {
            x: 0.0,
            y: 0.0,
            width: -800.0,
            height: 600.0,
        },
        PersistedWindowBounds {
            x: 0.0,
            y: 0.0,
            width: MAX_PLAUSIBLE_DIM * 10.0,
            height: 600.0,
        },
    ];

    for bounds in &cases {
        let config = AppConfig {
            window_bounds: Some(bounds.clone()),
            ..AppConfig::default()
        };
        assert_eq!(
            config.normalized().window_bounds,
            None,
            "implausible bounds {bounds:?} must fall back to default placement"
        );
    }
}

#[test]
fn normalized_keeps_plausible_and_multimonitor_bounds() {
    // A negative origin is legal (left-of-primary monitor), and a window
    // wider than the 8192 lint ceiling is still a real window worth keeping.
    let cases = [
        PersistedWindowBounds {
            x: -1920.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        },
        PersistedWindowBounds {
            x: 0.0,
            y: 0.0,
            width: 9000.0,
            height: 600.0,
        },
    ];
    for bounds in &cases {
        let config = AppConfig {
            window_bounds: Some(bounds.clone()),
            ..AppConfig::default()
        };
        assert!(
            config.normalized().window_bounds.is_some(),
            "plausible bounds {bounds:?} must survive normalization"
        );
    }
}
