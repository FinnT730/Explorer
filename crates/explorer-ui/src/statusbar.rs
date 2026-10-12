//! Status bar: item counts, selection size and free space on the volume.

use egui::{Align, Layout, Ui};
use explorer_core::Places;

use crate::action::Action;
use crate::settings::ViewMode;
use crate::tab::Tab;
use crate::theme::Palette;

pub fn show(ui: &mut Ui, tab: &Tab, places: &Places, pal: &Palette, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        let total = tab.view.len();
        let hidden = tab.entries.len() - total;
        let sel = tab.selection.len();

        let mut left = format!("{total} item{}", plural(total));
        if hidden > 0 {
            left.push_str(&format!("  ({hidden} filtered out)"));
        }
        if sel > 0 {
            left.push_str(&format!(
                "  |  {sel} selected  {}",
                explorer_core::fmt::size(tab.selection_size())
            ));
        }
        ui.label(egui::RichText::new(left).color(pal.text_dim));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if let Some(vol) = places.volume_for(tab.path()) {
                let text = format!(
                    "{} free of {}",
                    explorer_core::fmt::size(vol.available),
                    explorer_core::fmt::size(vol.total)
                );
                ui.label(egui::RichText::new(text).color(pal.text_dim))
                    .on_hover_text(format!("{} ({})", vol.mount.display(), vol.fs));
            }
            ui.separator();
            // Quick view switcher, like the buttons in the Explorer corner.
            for mode in [
                ViewMode::Tiles,
                ViewMode::Icons,
                ViewMode::List,
                ViewMode::Details,
            ] {
                let glyph = match mode {
                    ViewMode::Details => crate::icons::ViewGlyph::Details,
                    ViewMode::List => crate::icons::ViewGlyph::List,
                    ViewMode::Icons => crate::icons::ViewGlyph::Icons,
                    ViewMode::Tiles => crate::icons::ViewGlyph::Tiles,
                };
                let active = tab.view_mode == mode;
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(22.0, 18.0), egui::Sense::click());
                if active || resp.hovered() {
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(3),
                        if active {
                            pal.selection
                        } else {
                            ui.visuals().widgets.hovered.weak_bg_fill
                        },
                    );
                }
                let color = if active {
                    pal.selection_text
                } else {
                    pal.text_dim
                };
                crate::icons::view_glyph(ui.painter(), rect, glyph, color);
                if resp.on_hover_text(mode.label()).clicked() {
                    actions.push(Action::SetViewMode(mode));
                }
            }
            ui.separator();
            ui.label(
                egui::RichText::new(format!(
                    "{} on screen",
                    explorer_core::fmt::size(tab.total_size())
                ))
                .color(pal.text_dim),
            );
        });
    });
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
