use gpui::SharedString;

// No App context: gpui's test-support feature is a dev-dependency change,
// so drive load_themes_from_str (what register_embedded_themes calls).

/// (file, theme names) of every theme inside every embedded themes/*.json.
fn embedded_theme_names() -> Vec<(SharedString, Vec<SharedString>)> {
    crate::app::assets::embedded_themes()
        .into_iter()
        .map(|(file, json)| {
            let names = serde_json::from_str::<gpui_component::theme::ThemeSet>(&json)
                .unwrap_or_else(|err| panic!("{file}: {err}"))
                .themes
                .iter()
                .map(|theme| theme.name.clone())
                .collect::<Vec<_>>();
            (file, names)
        })
        .collect()
}

#[test]
fn embedded_theme_file_count_is_24() {
    // Tripwire: coverage in the tests below derives from the embed iterator;
    // this pins the shipped count so a dropped themes/*.json cannot slide by.
    assert_eq!(crate::app::assets::embedded_themes().len(), 24);
}

#[test]
fn every_embedded_theme_registers_and_resolves_by_name() {
    let mut registry = gpui_component::ThemeRegistry::default();
    for (file, json) in crate::app::assets::embedded_themes() {
        registry
            .load_themes_from_str(&json)
            .unwrap_or_else(|err| panic!("{file}: {err}"));
    }

    let mut expected = 0;
    for (file, names) in embedded_theme_names() {
        assert!(!names.is_empty(), "{file} carries no themes");
        expected += names.len();
        for name in names {
            assert!(
                registry.themes().contains_key(&name),
                "{file}: {name} missing from registry"
            );
        }
    }
    assert_eq!(
        registry.themes().len(),
        expected,
        "a duplicate theme name made load_themes_from_str silently skip it"
    );
}

#[test]
fn embedded_themes_do_not_collide_with_registry_defaults() {
    // load_themes_from_str skips already-present names; the real registry
    // boots with these two crate defaults, so an embedded name must differ.
    for (file, names) in embedded_theme_names() {
        for name in names {
            assert!(
                !matches!(name.as_str(), "Default Light" | "Default Dark"),
                "{file}: {name} collides with a registry default and would be skipped"
            );
        }
    }
}
