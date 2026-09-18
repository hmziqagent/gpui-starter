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

/// Wasm-only embedded assets: fonts + the gpui-kit default icon set.
///
/// Two wasm gaps this closes:
///
/// 1. **Fonts** — the wasm text system is created WITHOUT system fonts
///    (`CosmicTextSystem::new_without_system_fonts` — a browser tab cannot
///    enumerate installed fonts), so it starts empty and the FIRST text
///    layout would panic ("failed to resolve font ... or any of the
///    fallbacks"). `assets/fonts/NotoSans-Regular.ttf` (SIL OFL, license
///    alongside) is registered at boot via [`crate::app::init`]; its family
///    name is in gpui's default fallback stack, so every unresolved family
///    eventually lands on it.
///
/// 2. **Icons** — `gpui_kit_assets::Assets` on wasm is an on-demand CDN
///    loader: every icon requested before its fetch resolves logs a console
///    ERROR ("Wasm assets loading, will be available soon..."), and with no
///    `{endpoint}/assets/icons/` deployment those fetches 404 forever. The
///    native `Assets` unit struct embeds the 101-icon default set listed in
///    gpui-kit-assets' `default-icons.txt`; embedding the SAME set here
///    gives exact native parity, synchronously, with no CDN dependency.
#[cfg(target_family = "wasm")]
#[derive(rust_embed::RustEmbed)]
#[folder = "assets"]
struct WasmAssets;

/// Bytes of every embedded font (wasm-only; see [`WasmAssets`]). Only real
/// font files are returned — the folder also carries the OFL license text,
/// which fontdb rejects with a "malformed font" error if fed to it.
#[cfg(target_family = "wasm")]
pub fn embedded_font_bytes() -> Vec<Cow<'static, [u8]>> {
    WasmAssets::iter()
        .filter(|path| {
            (path.ends_with(".ttf") || path.ends_with(".otf")) && path.starts_with("fonts/")
        })
        .filter_map(|path| WasmAssets::get(&path).map(|file| file.data))
        .collect()
}

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
        // Wasm: embedded fonts + default icon set (see `WasmAssets`) —
        // checked synchronously BEFORE the on-demand CDN fallback so icons
        // resolve without a network round-trip.
        #[cfg(target_family = "wasm")]
        if let Some(file) = WasmAssets::get(path) {
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
