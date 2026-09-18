use tempfile::tempdir;

use super::{SqliteStorage, StorageBackend, init_db};

/// The trait is async (wasm answers via worker round-trips); the native
/// bodies resolve on their first poll, so a plain tokio runtime drives the
/// futures in tests (same pattern as the notification-backend tests).
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime for test")
        .block_on(future)
}

#[test]
fn initializes_schema_and_migration_table() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("app.db");
    let version = init_db(&db_path).expect("init db");
    assert_eq!(version, 3);

    let conn = rusqlite::Connection::open(&db_path).expect("open db");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = 3",
            [],
            |row| row.get(0),
        )
        .expect("read migrations");
    assert_eq!(count, 1);
}

#[test]
fn backend_health_and_maintenance_work() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("app.db");
    init_db(&db_path).expect("init db");
    let backend = SqliteStorage::new(db_path);
    block_on(backend.health_check()).expect("health check");
    block_on(backend.maintenance()).expect("maintenance");
    assert_eq!(
        block_on(backend.schema_version()).expect("schema version"),
        3
    );
}

#[test]
fn persist_and_load_crash_report_roundtrip() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("test_crash.db");
    init_db(&db_path).expect("init db");
    let backend = SqliteStorage::new(db_path);

    let report = crate::services::crash_report::CrashReport::new(
        "test panic".to_string(),
        "backtrace here".to_string(),
        true,
        vec!["error1".to_string(), "error2".to_string()],
    );

    block_on(backend.persist_crash_report(&report)).expect("persist crash report");

    let loaded = block_on(backend.load_pending_crash_reports(10)).expect("load pending");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, report.id);
    assert_eq!(loaded[0].panic_message, "test panic");
    assert_eq!(loaded[0].backtrace, "backtrace here");
    assert!(loaded[0].render_path);
    assert_eq!(loaded[0].recent_errors, vec!["error1", "error2"]);
}

#[test]
fn mark_crash_report_uploaded() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("test_upload.db");
    init_db(&db_path).expect("init db");
    let backend = SqliteStorage::new(db_path);

    let report = crate::services::crash_report::CrashReport::new(
        "panic".to_string(),
        "bt".to_string(),
        false,
        vec![],
    );

    block_on(backend.persist_crash_report(&report)).expect("persist");
    let pending = block_on(backend.load_pending_crash_reports(10)).expect("load");
    assert_eq!(pending.len(), 1);

    block_on(backend.mark_crash_report_uploaded(&report.id, "2025-01-01T00:00:00Z"))
        .expect("mark uploaded");

    let pending_after = block_on(backend.load_pending_crash_reports(10)).expect("load after");
    assert_eq!(pending_after.len(), 0);
}

#[test]
fn load_pending_crash_reports_respects_limit() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("test_limit.db");
    init_db(&db_path).expect("init db");
    let backend = SqliteStorage::new(db_path);

    for _ in 0..5 {
        let report = crate::services::crash_report::CrashReport::new(
            "panic".to_string(),
            "bt".to_string(),
            false,
            vec![],
        );
        block_on(backend.persist_crash_report(&report)).expect("persist");
    }

    let loaded = block_on(backend.load_pending_crash_reports(3)).expect("load");
    assert_eq!(loaded.len(), 3);
}
