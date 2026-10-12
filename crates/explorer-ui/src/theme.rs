//! Visual style. Deliberately flat and dense, closer to Explorer than to the
//! stock egui look: tight rows, quiet separators, one accent colour.

use egui::{Color32, CornerRadius, Stroke, Visuals};

use crate::settings::Theme;

pub const ACCENT: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);

pub struct Palette {
    pub accent: Color32,
    pub folder: Color32,
    pub text_dim: Color32,
    pub row_alt: Color32,
    pub separator: Color32,
    pub selection: Color32,
    pub selection_text: Color32,
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            accent: ACCENT,
            folder: Color32::from_rgb(0xf5, 0xc0, 0x5e),
            text_dim: Color32::from_gray(150),
            row_alt: Color32::from_rgba_unmultiplied(255, 255, 255, 6),
            separator: Color32::from_gray(60),
            selection: Color32::from_rgb(0x1d, 0x4e, 0xd8),
            selection_text: Color32::WHITE,
        }
    } else {
        Palette {
            accent: ACCENT,
            folder: Color32::from_rgb(0xe0, 0xa8, 0x2e),
            text_dim: Color32::from_gray(100),
            row_alt: Color32::from_rgba_unmultiplied(0, 0, 0, 8),
            separator: Color32::from_gray(200),
            selection: Color32::from_rgb(0xbf, 0xd9, 0xff),
            selection_text: Color32::BLACK,
        }
    }
}

pub fn is_dark(theme: Theme, ctx: &egui::Context) -> bool {
    match theme {
        Theme::Dark => true,
        Theme::Light => false,
        Theme::System => ctx.system_theme().unwrap_or(egui::Theme::Dark) == egui::Theme::Dark,
    }
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let dark = is_dark(theme, ctx);
    let pal = palette(dark);
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.selection.bg_fill = pal.selection;
    visuals.selection.stroke = Stroke::new(1.0, pal.accent);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.separator);
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.popup_shadow.offset = [1, 3];
    visuals.popup_shadow.blur = 12;
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(6.0, 3.0);
        style.spacing.menu_margin = egui::Margin::same(4);
        style.spacing.scroll.bar_width = 10.0;
        style.interaction.selectable_labels = false;
        style.interaction.multi_widget_text_select = false;
    });
}
