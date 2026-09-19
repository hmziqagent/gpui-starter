//! Clipboard write helpers.
//!
//! Thin, fallible wrappers around [`arboard::Clipboard`] for setting text
//! and image content. Failures are surfaced as [`ClipboardError`] (a
//! dedicated error variant) rather than panicking, matching the
//! boilerplate's "return Result" convention.
//!
//! Wasm: `arboard` has no wasm32-unknown-unknown backend, so text writes go
//! through `navigator.clipboard.writeText` instead (fire-and-forget — see
//! [`crate::platform::web::clipboard`]); image writes degrade to an explicit
//! error.

use std::fmt;

#[cfg(not(target_family = "wasm"))]
use arboard::{Clipboard, ImageData};

use super::item::ClipboardContent;

const LOG: &str = "gpui_starter::clipboard::copy";

/// Errors that can occur while writing to the system clipboard.
#[derive(Debug)]
pub enum ClipboardError {
    /// The platform clipboard could not be opened.
    AccessFailed(String),
    /// The write (set text/image) was rejected by the clipboard.
    WriteFailed(String),
}

impl fmt::Display for ClipboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessFailed(details) => {
                write!(f, "clipboard access failed: {details}")
            }
            Self::WriteFailed(details) => {
                write!(f, "clipboard write failed: {details}")
            }
        }
    }
}

impl std::error::Error for ClipboardError {}

#[cfg(not(target_family = "wasm"))]
impl From<arboard::Error> for ClipboardError {
    fn from(err: arboard::Error) -> Self {
        Self::AccessFailed(err.to_string())
    }
}

#[cfg(target_family = "wasm")]
fn unavailable() -> ClipboardError {
    ClipboardError::AccessFailed("clipboard image write unsupported on wasm".to_string())
}

/// Write plain text to the system clipboard.
#[cfg(not(target_family = "wasm"))]
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    let mut clipboard =
        Clipboard::new().map_err(|err| ClipboardError::AccessFailed(err.to_string()))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| ClipboardError::WriteFailed(err.to_string()))?;
    tracing::debug!(target: LOG, len = text.len(), "wrote text to clipboard");
    Ok(())
}

/// Write an RGBA image to the system clipboard.
///
/// `rgba_bytes` must be `width * height * 4` bytes long; callers are
/// responsible for ensuring this invariant (an inconsistency surfaces as a
/// [`ClipboardError::WriteFailed`]).
#[cfg(not(target_family = "wasm"))]
pub fn set_image(width: usize, height: usize, rgba_bytes: &[u8]) -> Result<(), ClipboardError> {
    let mut clipboard =
        Clipboard::new().map_err(|err| ClipboardError::AccessFailed(err.to_string()))?;
    let image_data = ImageData {
        width,
        height,
        bytes: std::borrow::Cow::Borrowed(rgba_bytes),
    };
    clipboard
        .set_image(image_data)
        .map_err(|err| ClipboardError::WriteFailed(err.to_string()))?;
    tracing::debug!(
        target: LOG,
        width, height, bytes = rgba_bytes.len(),
        "wrote image to clipboard"
    );
    Ok(())
}

/// Wasm: `arboard` has no browser backend — write text through the async
/// `navigator.clipboard.writeText` API instead. The sync `-> Result` shape
/// cannot observe the Promise, so the write is fire-and-forget: rejections
/// are logged (tracing) by the bridge and `Ok` is returned immediately.
#[cfg(target_family = "wasm")]
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    crate::platform::web::clipboard::write_text_fire_and_forget(text);
    tracing::debug!(target: LOG, len = text.len(), "clipboard text write dispatched on wasm");
    Ok(())
}

/// Wasm: `navigator.clipboard.write(ClipboardItem)` is PNG +
/// secure-context-only with no raw-RGB path — image writes degrade to an
/// explicit error (parity with the arboard-less stub it replaces).
#[cfg(target_family = "wasm")]
pub fn set_image(_width: usize, _height: usize, _rgba_bytes: &[u8]) -> Result<(), ClipboardError> {
    tracing::debug!(target: LOG, "clipboard image write skipped on wasm");
    Err(unavailable())
}

/// Write arbitrary [`ClipboardContent`] to the clipboard.
///
/// Convenience dispatcher used by both the copy hot-path and (feature-gated)
/// re-publish of a history entry.
pub fn set_content(content: &ClipboardContent) -> Result<(), ClipboardError> {
    match content {
        ClipboardContent::Text(text) => set_text(text),
        ClipboardContent::Image {
            width,
            height,
            rgba_bytes,
        } => set_image(*width, *height, rgba_bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_is_human_readable() {
        let err = ClipboardError::AccessFailed("boom".into());
        assert!(err.to_string().contains("boom"));
        let err = ClipboardError::WriteFailed("nope".into());
        assert!(err.to_string().contains("nope"));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn arboard_error_converts() {
        let err = ClipboardError::from(arboard::Error::ContentNotAvailable);
        assert!(matches!(err, ClipboardError::AccessFailed(_)));
    }
}
