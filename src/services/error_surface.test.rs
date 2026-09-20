use super::*;

#[test]
fn error_category_labels() {
    assert_eq!(ErrorCategory::Network.label(), "network");
    assert_eq!(ErrorCategory::Storage.label(), "storage");
    assert_eq!(ErrorCategory::Rendering.label(), "rendering");
    assert_eq!(ErrorCategory::Config.label(), "config");
    assert_eq!(ErrorCategory::System.label(), "system");
}

#[test]
fn error_record_serializes_with_category() {
    let record = ErrorRecord {
        id: EventId::new(),
        occurred_at: AppTimestamp::now(),
        severity: crate::errors::AppErrorSeverity::Warning,
        category: ErrorCategory::Network,
        message: "connection timed out".into(),
        actions: vec![ErrorAction::Retry, ErrorAction::Dismiss],
    };
    let json = serde_json::to_string(&record).expect("serialize");
    assert!(json.contains("Network"));
    let deserialized: ErrorRecord = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deserialized.category, ErrorCategory::Network);
}

#[test]
fn scrub_home_replaces_home_prefix() {
    assert_eq!(
        scrub_home_with("/home/me/logs/x.log", "/home/me"),
        "~/logs/x.log"
    );
    // Replacement is textual (matches the crash_report scrub), so a shared
    // prefix without a path boundary is also rewritten — still scrubbed.
    assert_eq!(scrub_home_with("/home/me2/x", "/home/me"), "~2/x");
    assert_eq!(scrub_home_with("plain text", ""), "plain text");
}

#[test]
fn persist_and_load_roundtrip() {
    use crate::storage::{SqliteStorage, StorageBackend as _};

    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test_errors.db");

    // Create the error_log table inline (mirrors migration v2).
    {
        let conn = rusqlite::Connection::open(&db_path).expect("open");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS error_log (
                id TEXT PRIMARY KEY,
                occurred_at TEXT NOT NULL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                message TEXT NOT NULL,
                actions TEXT NOT NULL
            );",
        )
        .expect("create table");
    }

    let backend = SqliteStorage::new_for_test(db_path);

    let record = ErrorRecord {
        id: EventId::new(),
        occurred_at: AppTimestamp::now(),
        severity: crate::errors::AppErrorSeverity::Error,
        category: ErrorCategory::Storage,
        message: "disk full".into(),
        actions: vec![ErrorAction::Dismiss],
    };

    // persist_error/load_error_history are async (the wasm backend answers
    // via worker round-trips); the native body resolves on first poll.
    let runtime = || tokio::runtime::Runtime::new().expect("tokio runtime for test");
    runtime()
        .block_on(persist_error(&backend, &record))
        .expect("persist");
    let loaded = runtime()
        .block_on(backend.load_error_history(10))
        .expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].message, "disk full");
    assert_eq!(loaded[0].category, ErrorCategory::Storage);
    assert_eq!(loaded[0].severity, crate::errors::AppErrorSeverity::Error);
}

#[test]
fn load_error_history_respects_limit() {
    use crate::storage::{SqliteStorage, StorageBackend as _};

    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("test_limit.db");

    {
        let conn = rusqlite::Connection::open(&db_path).expect("open");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS error_log (
                id TEXT PRIMARY KEY,
                occurred_at TEXT NOT NULL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                message TEXT NOT NULL,
                actions TEXT NOT NULL
            );",
        )
        .expect("create table");
    }

    let backend = SqliteStorage::new_for_test(db_path);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime for test");

    for i in 0..5 {
        let record = ErrorRecord {
            id: EventId::new(),
            occurred_at: AppTimestamp::now(),
            severity: crate::errors::AppErrorSeverity::Info,
            category: ErrorCategory::System,
            message: format!("error {i}"),
            actions: vec![],
        };
        runtime
            .block_on(persist_error(&backend, &record))
            .expect("persist");
    }

    let loaded = runtime
        .block_on(backend.load_error_history(3))
        .expect("load");
    assert_eq!(loaded.len(), 3);
}
