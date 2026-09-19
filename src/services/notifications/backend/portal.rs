//! XDG Desktop Portal notification backend (Flatpak / Snap / sandboxed runs),
//! enabled with the `notifications-portal` feature and selected automatically
//! when [`crate::platform::environment::is_sandboxed`] is true.
//!
//! ashpd is built on zbus/async-io (NOT tokio — zbus forbids both). zbus
//! drives its own background thread, so these futures are safe to await
//! directly in the async trait, exactly like notify-rust's `show_async`.

#![cfg(feature = "notifications-portal")]

use async_trait::async_trait;

use super::NotificationBackend;
use crate::notifications::{
    DESKTOP_ENTRY_ID, NotificationBackendKind, NotificationCapabilities,
    NotificationPermissionState, NotificationRequest,
};

const LOG: &str = "gpui_starter::notifications::portal";

/// Portal notification backend (no runtime handle needed — ashpd drives
/// zbus's own async-io executor thread).
pub struct PortalBackend;

impl PortalBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NotificationBackend for PortalBackend {
    fn kind(&self) -> NotificationBackendKind {
        NotificationBackendKind::Portal
    }

    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            can_request_permission: false,
            can_read_permission_state: false,
            can_send_immediate_native: true,
            can_send_interactive: false,
            // The portal is the correct path inside a packaged (Flatpak/Snap) runtime.
            requires_packaged_runtime: true,
        }
    }

    async fn refresh_permission_state(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn request_permission(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()> {
        tracing::info!(target: LOG, title = %request.title, "sending notification through the XDG portal");

        // A successful AddNotification only means the portal accepted it —
        // there is no display confirmation.
        let proxy = ashpd::desktop::notification::NotificationProxy::new().await?;
        let notification = ashpd::desktop::notification::Notification::new(&request.title)
            .body(request.body.as_str());
        match proxy.add_notification(DESKTOP_ENTRY_ID, notification).await {
            Ok(()) => {
                tracing::info!(target: LOG, "portal notification accepted");
                Ok(())
            }
            Err(err) => {
                tracing::warn!(target: LOG, error = %err, "portal notification send failed");
                Err(err.into())
            }
        }
    }
}
