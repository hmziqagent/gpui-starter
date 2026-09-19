//! Persistent storage service: the target-neutral [`StorageBackend`] trait,
//! its per-target implementations, and the boot/runtime surface in
//! [`runtime`].
//!
//! - Native: [`backend::SqliteStorage`] holds one shared `rusqlite`
//!   connection behind a mutex.
//! - Wasm: [`web::WebSqliteStorage`] talks to the OPFS sqlite worker
//!   (`wasm/sqlite/worker.js`, built on the vendored
//!   `@sqlite.org/sqlite-wasm` engine) over the JSON protocol in
//!   [`protocol`] and falls back to the unavailable-snapshot behavior when
//!   OPFS or the Worker API is missing.
//!
//! The trait is async ([`async_trait`], same shape as
//! `NotificationBackend`): the wasm backend can only answer via worker
//! message round-trips. Native bodies stay synchronous internally — no
//! await points — so SQLite I/O still completes without ever yielding.
#[cfg(not(target_family = "wasm"))]
mod backend;
pub(crate) mod protocol;
mod runtime;
#[cfg(target_family = "wasm")]
mod web;

#[cfg(test)]
#[cfg(not(target_family = "wasm"))]
pub(crate) use backend::SqliteStorage;
pub use runtime::*;

use async_trait::async_trait;
use gpui::Global;

#[derive(Clone, Debug, Default)]
pub struct StorageSnapshot {
    pub available: bool,
    pub db_path: String,
    pub schema_version: i64,
    pub healthy: bool,
    pub last_maintenance_at: Option<String>,
    pub last_migration_result: Option<String>,
    pub last_error: Option<String>,
}

impl Global for StorageSnapshot {}

/// Target-neutral storage error.
///
/// `rusqlite::Error` only exists natively (rusqlite is a not-wasm dep), so
/// it is wrapped in a variant compiled out on wasm; the web backend reports
/// its transport/OPFS/SQL failures through [`StorageError::Web`] with a
/// human-readable message. Callers format both the same way.
#[derive(Debug)]
pub enum StorageError {
    /// Native SQLite failure.
    #[cfg(not(target_family = "wasm"))]
    Sqlite(rusqlite::Error),
    /// Web backend failure: worker transport, OPFS, or worker-side SQL
    /// error text.
    Web(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(not(target_family = "wasm"))]
            Self::Sqlite(err) => write!(f, "{err}"),
            Self::Web(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            #[cfg(not(target_family = "wasm"))]
            Self::Sqlite(err) => Some(err),
            Self::Web(_) => None,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err)
    }
}

// async_trait's generated wrapper fns carry #[must_use] and return boxed
// futures (already #[must_use]) — new nightly clippy flags the generated
// code as double_must_use. The attribute is macro-emitted, so the allow
// lives here at the macro use site (same as NotificationBackend).
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn schema_version(&self) -> Result<i64, StorageError>;
    async fn health_check(&self) -> Result<(), StorageError>;
    async fn maintenance(&self) -> Result<(), StorageError>;
    async fn persist_error_record(
        &self,
        error: &crate::error_surface::ErrorRecord,
    ) -> Result<(), StorageError>;
    async fn load_error_history(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::error_surface::ErrorRecord>, StorageError>;
    async fn persist_crash_report(
        &self,
        report: &crate::services::crash_report::CrashReport,
    ) -> Result<(), StorageError>;
    async fn load_pending_crash_reports(
        &self,
        limit: usize,
    ) -> Result<Vec<crate::services::crash_report::CrashReport>, StorageError>;
    async fn mark_crash_report_uploaded(
        &self,
        id: &str,
        uploaded_at: &str,
    ) -> Result<(), StorageError>;
}

#[cfg(test)]
#[path = "../storage.test.rs"]
mod storage_test;
