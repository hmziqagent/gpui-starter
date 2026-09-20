use gpui::App;
use gpui_component::ActiveTheme as _;

/// Register the compile-time-embedded themes into the global registry. Runs
/// unconditionally at startup, and again after a watcher reload (which clears
/// the registry); name-dedup keeps already-registered dev themes winning.
pub fn register_embedded_themes(cx: &mut App) {
    let registry = gpui_component::ThemeRegistry::global_mut(cx);
    for (name, json) in crate::app::assets::embedded_themes() {
        if let Err(err) = registry.load_themes_from_str(&json) {
            tracing::warn!("invalid embedded theme {name}: {err}");
        }
    }
}

pub fn set_theme_mode(mode: gpui_component::ThemeMode, cx: &mut App) {
    set_theme_mode_with_record(mode, true, cx);
}

pub fn set_theme_mode_with_record(mode: gpui_component::ThemeMode, record: bool, cx: &mut App) {
    let before = cx.theme().mode;
    gpui_component::Theme::change(mode, None, cx);
    if record {
        crate::undo_stack::record_theme_mode_change(before, mode, cx);
    }
    cx.refresh_windows();
}

#[cfg(test)]
#[path = "theme.test.rs"]
mod theme_test;
