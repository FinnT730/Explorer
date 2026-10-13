//! Visual style, modelled on the Windows 11 File Explorer (Fluent): a quiet
//! layered background, 4 px rounded highlights, an accent-tinted selection and
//! dense-but-breathable rows.
//!
//! The palette is expressed as tokens so the same widget code can sit on an
//! opaque background or on a translucent Mica/acrylic backdrop.

use egui::{Color32, CornerRadius, Stroke, Visuals};

use crate::settings::Theme;

/// Windows' own default accent (`#0078D4`), and its dark-mode highlight.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x00, 0x67, 0xc0);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x4c, 0xc2, 0xff);

pub struct Palette {
    pub dark: bool,
    /// Accent for focus rings, selection tint and active affordances.
    pub accent: Color32,
    /// Chrome behind the tab strip and toolbars (shows the backdrop).
    pub chrome: Color32,
    /// The listing surface, one layer above the chrome.
    pub surface: Color32,
    /// Inputs and chips sitting on the chrome.
    pub field: Color32,
    pub field_stroke: Color32,
    pub folder: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub row_alt: Color32,
    pub separator: Color32,
    pub hover: Color32,
    /// Selected row fill: accent, heavily tinted down.
    pub selection: Color32,
    pub selection_text: Color32,
    pub danger: Color32,
}

/// Fill colours lose their alpha when the window is opaque, so the same tokens
/// work with and without a backdrop.
pub fn palette(dark: bool, translucent: bool) -> Palette {
    let a = |c: Color32, alpha: u8| {
        if translucent {
            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
        } else {
            c
        }
    };
    if dark {
        Palette {
            dark,
            accent: ACCENT_DARK,
            chrome: a(Color32::from_rgb(0x20, 0x20, 0x20), 0),
            surface: a(Color32::from_rgb(0x27, 0x27, 0x27), 225),
            field: Color32::from_rgb(0x2d, 0x2d, 0x2d),
            field_stroke: Color32::from_rgb(0x3d, 0x3d, 0x3d),
            folder: Color32::from_rgb(0xf6, 0xc9, 0x5c),
            text: Color32::from_rgb(0xf2, 0xf2, 0xf2),
            text_dim: Color32::from_rgb(0x9c, 0x9c, 0x9c),
            row_alt: Color32::from_rgba_unmultiplied(255, 255, 255, 5),
            separator: Color32::from_rgb(0x3a, 0x3a, 0x3a),
            hover: Color32::from_rgba_unmultiplied(255, 255, 255, 14),
            selection: Color32::from_rgba_unmultiplied(0x4c, 0xc2, 0xff, 38),
            selection_text: Color32::from_rgb(0xff, 0xff, 0xff),
            danger: Color32::from_rgb(0xe8, 0x62, 0x5a),
        }
    } else {
        Palette {
            dark,
            accent: ACCENT_LIGHT,
            chrome: a(Color32::from_rgb(0xf3, 0xf3, 0xf3), 0),
            surface: a(Color32::from_rgb(0xff, 0xff, 0xff), 232),
            field: Color32::from_rgb(0xfd, 0xfd, 0xfd),
            field_stroke: Color32::from_rgb(0xd8, 0xd8, 0xd8),
            folder: Color32::from_rgb(0xf7, 0xc8, 0x55),
            text: Color32::from_rgb(0x1b, 0x1b, 0x1b),
            text_dim: Color32::from_rgb(0x5d, 0x5d, 0x5d),
            row_alt: Color32::from_rgba_unmultiplied(0, 0, 0, 6),
            separator: Color32::from_rgb(0xe2, 0xe2, 0xe2),
            hover: Color32::from_rgba_unmultiplied(0, 0, 0, 12),
            selection: Color32::from_rgba_unmultiplied(0x00, 0x67, 0xc0, 32),
            selection_text: Color32::from_rgb(0x10, 0x10, 0x10),
            danger: Color32::from_rgb(0xc4, 0x2b, 0x1c),
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

/// Corner radius used for rows, buttons and chips, as Fluent specifies.
pub const RADIUS_SMALL: u8 = 4;
pub const RADIUS_CHIP: u8 = 6;

pub fn apply(ctx: &egui::Context, theme: Theme, translucent: bool) {
    let dark = is_dark(theme, ctx);
    let pal = palette(dark, translucent);
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = pal.chrome;
    visuals.window_fill = if dark {
        Color32::from_rgb(0x2b, 0x2b, 0x2b)
    } else {
        Color32::from_rgb(0xf9, 0xf9, 0xf9)
    };
    visuals.override_text_color = Some(pal.text);
    visuals.selection.bg_fill = pal.selection;
    visuals.selection.stroke = Stroke::new(1.0, pal.accent);
    visuals.hyperlink_color = pal.accent;

    // Flat Fluent widgets: no bevels, a hairline border, rounded corners.
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(RADIUS_SMALL);
        w.bg_stroke = Stroke::NONE;
        w.expansion = 0.0;
    }
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_SMALL);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, pal.separator);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    visuals.widgets.hovered.weak_bg_fill = pal.hover;
    visuals.widgets.hovered.bg_fill = pal.hover;
    visuals.widgets.active.weak_bg_fill = pal.hover;
    visuals.widgets.active.bg_fill = pal.hover;
    visuals.widgets.open.weak_bg_fill = pal.hover;

    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, pal.separator);
    visuals.popup_shadow.offset = [0, 4];
    visuals.popup_shadow.blur = 18;
    visuals.popup_shadow.spread = 0;
    visuals.window_shadow.offset = [0, 8];
    visuals.window_shadow.blur = 28;
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(6.0, 4.0);
        style.spacing.button_padding = egui::vec2(9.0, 5.0);
        style.spacing.menu_margin = egui::Margin::symmetric(4, 6);
        style.spacing.window_margin = egui::Margin::same(14);
        style.spacing.scroll.bar_width = 10.0;
        style.spacing.scroll.floating = true;
        style.spacing.menu_width = 220.0;
        style.interaction.selectable_labels = false;
        style.interaction.multi_widget_text_select = false;
        style.visuals.indent_has_left_vline = false;
    });
}

/// Pick the font closest to the platform's UI font. Windows and macOS ship one
/// the user already reads all day; elsewhere egui's bundled face is fine.
pub fn install_system_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        #[cfg(windows)]
        "C:\\Windows\\Fonts\\SegoeUIVariableStatic-Regular.ttf",
        #[cfg(windows)]
        "C:\\Windows\\Fonts\\segoeui.ttf",
        #[cfg(target_os = "macos")]
        "/System/Library/Fonts/SFNS.ttf",
        #[cfg(target_os = "macos")]
        "/Library/Fonts/SF-Pro.ttf",
    ];

    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "ui".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "ui".to_owned());
        ctx.set_fonts(fonts);
        return;
    }
}
