//! Selection state, fuzzy filtering, and the entry model behind the
//! command palette in [`crate::features::command_palette`].

mod base_delegate;
mod filter;
mod items;

pub use base_delegate::BaseDelegate;
pub use filter::{FuzzyMatchConfig, ItemFilter};
pub use items::PaletteEntry;
