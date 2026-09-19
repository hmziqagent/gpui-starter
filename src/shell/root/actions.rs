use gpui::Action;

/// Navigate directly to a sidebar page by index (0-based).
#[derive(Action, Clone, PartialEq, Eq, serde::Deserialize)]
#[action(namespace = app, no_json)]
pub struct NavigateToPage(pub usize);

/// Re-navigate to the current page (triggers a route refresh).
#[derive(Action, Clone, PartialEq, Eq, serde::Deserialize)]
#[action(namespace = app, no_json)]
pub struct RefreshPage;

/// True for RTL script locales: Arabic, Hebrew, Farsi, Urdu.
pub(crate) fn is_rtl_locale(locale: &str) -> bool {
    locale
        .split('-')
        .next()
        .map(|primary| matches!(primary, "ar" | "he" | "fa" | "ur"))
        .unwrap_or(false)
}
