//! The contract every palette row must satisfy to be scored by [`crate::features::palette::ItemFilter`].

/// A selectable, filterable palette row. `name` is the primary search target;
/// `description` is a fallback target shown as the secondary line.
pub trait PaletteEntry {
    /// Primary label, also the primary fuzzy-match target.
    fn name(&self) -> &str;

    /// Optional secondary line / fallback match target.
    fn description(&self) -> Option<&str> {
        None
    }
}
