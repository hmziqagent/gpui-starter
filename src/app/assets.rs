//! Combined asset source.
//!
//! Merges the bundled gpui-kit icon assets (`gpui_kit_assets::Assets`)
//! with gpui-starter's own project assets (currently the shipped theme
//! definitions under `themes/`). Project assets take precedence on lookup so
//! an app-supplied file shadows a same-named component asset. This replaces
//! the bare `with_assets(Assets)` call in `main.rs` so the binary carries
//! both sets of resources.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// Project-local embedded assets (theme JSON shipped under `themes/`).
///
/// The folder path is deliberately RELATIVE: without rust-embed's
/// `interpolate-folder-path` feature, `$CARGO_MANIFEST_DIR/themes` is treated
/// as a literal (nonexistent) path — which compiles in dev-native (runtime
/// fs mode) but breaks every build that embeds at compile time (release
/// native and wasm). A relative folder is resolved against the crate root by
/// rust-embed and embeds correctly everywhere.
#[derive(rust_embed::RustEmbed)]
#[folder = "themes"]
struct ProjectAssets;

/// A merged [`AssetSource`] combining gpui-kit assets with project assets.
pub struct CombinedAssets {
    component: gpui_kit_assets::Assets,
}

impl CombinedAssets {
    pub fn new() -> Self {
        Self {
            // Native: RustEmbed unit struct (icons embedded in the binary).
            // Wasm: gpui-kit-assets swaps to an on-demand CDN source (empty
            // endpoint = relative to the served page).
            #[cfg(not(target_family = "wasm"))]
            component: gpui_kit_assets::Assets,
            #[cfg(target_family = "wasm")]
            component: gpui_kit_assets::Assets::default(),
        }
    }
}

impl Default for CombinedAssets {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSource for CombinedAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        // Project assets take precedence.
        if let Some(file) = ProjectAssets::get(path) {
            return Ok(Some(file.data));
        }
        // Fall back to the bundled component assets (icons, etc.).
        self.component.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut entries: Vec<SharedString> = ProjectAssets::iter()
            .filter(|p| p.starts_with(path))
            .map(Into::into)
            .collect();
        let mut component_entries = self.component.list(path)?;
        entries.append(&mut component_entries);
        Ok(entries)
    }
}
