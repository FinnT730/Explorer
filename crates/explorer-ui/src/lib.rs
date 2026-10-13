//! Explorer's user interface: an eframe/egui application.

pub mod action;
pub mod app;
pub mod backdrop;
pub mod dialogs;
pub mod icons;
pub mod preview;
pub mod settings;
pub mod sidebar;
pub mod statusbar;
pub mod tab;
pub mod textures;
pub mod theme;
pub mod toolbar;
pub mod views;

pub use app::App;

/// Number of leading characters of a file name that a rename box should
/// pre-select: everything before the extension.
pub fn selectable_stem(name: &str) -> usize {
    let (stem, ext) = explorer_core::ops::split_name(name);
    if ext.is_empty() {
        name.chars().count()
    } else {
        stem.chars().count()
    }
}
