#[cfg(not(target_family = "wasm"))]
mod notify_rust;
#[cfg(feature = "notifications-portal")]
mod portal;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod user_notify;

#[cfg(not(target_family = "wasm"))]
pub use notify_rust::NotifyRustBackend;
#[cfg(feature = "notifications-portal")]
pub use portal::PortalBackend;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub use user_notify::UserNotifyBackend;

use async_trait::async_trait;

use crate::notifications::{
    NotificationBackendKind, NotificationCapabilities, NotificationPermissionState,
    NotificationRequest,
};

// async_trait's generated wrapper fns carry #[must_use] and return boxed
// futures (already #[must_use]) — new nightly clippy flags the generated code
// as double_must_use. The attribute is macro-emitted, so the allow lives here
// at the macro use site.
#[allow(clippy::double_must_use)]
#[async_trait]
pub trait NotificationBackend: Send + Sync {
    fn kind(&self) -> NotificationBackendKind;
    fn capabilities(&self) -> NotificationCapabilities;
    async fn refresh_permission_state(&self) -> NotificationPermissionState;
    async fn request_permission(&self) -> NotificationPermissionState;
    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()>;
}

/// Wasm: there is no OS notification daemon. This no-op backend takes the
/// secondary slot so the service keeps its primary/secondary shape; every
/// send fails and the notification service falls back to the in-app toast +
/// inbox policy (pure gpui, works in the browser).
#[cfg(target_family = "wasm")]
pub struct WasmStubBackend;

#[cfg(target_family = "wasm")]
impl WasmStubBackend {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_family = "wasm")]
impl Default for WasmStubBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_family = "wasm")]
#[async_trait]
impl NotificationBackend for WasmStubBackend {
    fn kind(&self) -> NotificationBackendKind {
        NotificationBackendKind::UiOnly
    }

    fn capabilities(&self) -> NotificationCapabilities {
        NotificationCapabilities {
            can_request_permission: false,
            can_read_permission_state: false,
            can_send_immediate_native: false,
            can_send_interactive: false,
            requires_packaged_runtime: false,
        }
    }

    async fn refresh_permission_state(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn request_permission(&self) -> NotificationPermissionState {
        NotificationPermissionState::Unsupported
    }

    async fn send(&self, request: &NotificationRequest) -> anyhow::Result<()> {
        anyhow::bail!(
            "native notifications unavailable on wasm (title: {})",
            request.title
        );
    }
}
