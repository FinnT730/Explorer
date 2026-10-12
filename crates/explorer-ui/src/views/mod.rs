//! The four listing views and the pieces they share.

pub mod details;
pub mod grid;
pub mod list;

use egui::{Align2, Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use explorer_core::{ops::Clipboard, Entry};
use explorer_thumbs::{State as ThumbState, Thumbnails};

use crate::action::Action;
use crate::icons;
use crate::settings::{Settings, ViewMode};
use crate::tab::Tab;
use crate::textures::TextureStore;
use crate::theme::Palette;

/// Everything a view needs besides the tab itself.
pub struct ViewCtx<'a> {
    pub settings: &'a Settings,
    pub pal: &'a Palette,
    pub thumbs: &'a Thumbnails,
    pub textures: &'a mut TextureStore,
    pub clipboard: &'a Clipboard,
    pub actions: &'a mut Vec<Action>,
}

pub fn show(ui: &mut Ui, tab: &mut Tab, ctx: &mut ViewCtx<'_>) {
    if let Some(err) = tab.error.clone() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(egui::RichText::new("Can't open this folder").strong());
            ui.label(egui::RichText::new(err).color(ctx.pal.text_dim));
            if ui.button("Go up").clicked() {
                ctx.actions.push(Action::Up);
            }
        });
        return;
    }

    if tab.view.is_empty() {
        let msg = if tab.is_loading() {
            "Working…"
        } else if tab.filter.is_searching() {
            "No items match your search"
        } else if tab.entries.is_empty() {
            "This folder is empty"
        } else {
            "No items match the current filter"
        };
        // Still draw an interactive background so right-click and paste work.
        let rect = ui.available_rect_before_wrap();
        let resp = ui.interact(rect, ui.id().with("empty-bg"), Sense::click());
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            msg,
            FontId::proportional(13.0),
            ctx.pal.text_dim,
        );
        background_menu(&resp, ctx);
        return;
    }

    // One background behind every view: registered first, so items drawn on
    // top of it still win the click, and empty space below the last row
    // still opens the folder menu.
    let bg_rect = ui.available_rect_before_wrap();
    let bg = ui.interact(bg_rect, ui.id().with("listing-bg"), Sense::click());

    match tab.view_mode {
        ViewMode::Details => details::show(ui, tab, ctx),
        ViewMode::List => list::show(ui, tab, ctx),
        ViewMode::Icons => grid::show(ui, tab, ctx, false),
        ViewMode::Tiles => grid::show(ui, tab, ctx, true),
    }

    background_menu(&bg, ctx);
    if bg.clicked() {
        ctx.actions.push(Action::ClearSelection);
    }
}

/// Paint the selection/hover backdrop for one item.
pub fn paint_row_background(
    ui: &Ui,
    rect: Rect,
    selected: bool,
    cursored: bool,
    hovered: bool,
    pal: &Palette,
) {
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, CornerRadius::same(4), pal.selection);
    } else if hovered {
        painter.rect_filled(
            rect,
            CornerRadius::same(4),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    if cursored {
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, pal.accent),
            StrokeKind::Inside,
        );
    }
}

pub fn text_color(selected: bool, pal: &Palette, ui: &Ui) -> Color32 {
    if selected {
        pal.selection_text
    } else {
        ui.visuals().text_color()
    }
}

/// Draw an entry's icon, substituting a decoded thumbnail when we have one.
pub fn draw_icon(ui: &mut Ui, rect: Rect, entry: &Entry, ctx: &mut ViewCtx<'_>) {
    let want_thumb = ctx.settings.thumbnails
        && entry.class.is_previewable_image()
        && rect.width() >= 14.0
        && !entry.is_dir;

    if want_thumb {
        let bucket = explorer_thumbs::bucket_for(rect.width().ceil() as u32);
        match ctx.thumbs.get(&entry.path, entry.modified, bucket) {
            ThumbState::Ready(thumb) => {
                let tex = ctx.textures.get(ui.ctx(), &entry.path, bucket, &thumb);
                let size = fit(
                    egui::vec2(thumb.width as f32, thumb.height as f32),
                    rect.size(),
                );
                let target = Rect::from_center_size(rect.center(), size);
                // Thin frame so light photos stay distinguishable from the row.
                ui.painter().rect_filled(
                    target.expand(1.0),
                    CornerRadius::same(2),
                    ctx.pal.separator,
                );
                ui.painter().image(
                    tex.id(),
                    target,
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                return;
            }
            ThumbState::Pending => {
                // Fall through to the vector icon; the thumbnail swaps in when
                // it lands, so the grid never shows an empty hole.
            }
            ThumbState::Failed | ThumbState::Missing => {}
        }
    }

    icons::draw(
        ui.painter(),
        rect,
        entry.class,
        &entry.ext,
        ctx.pal,
        entry.is_symlink,
    );
}

/// Scale `src` to fit inside `dst` without distortion.
pub fn fit(src: egui::Vec2, dst: egui::Vec2) -> egui::Vec2 {
    if src.x <= 0.0 || src.y <= 0.0 {
        return dst;
    }
    let f = (dst.x / src.x).min(dst.y / src.y).min(1.0e6);
    src * f
}

/// Translate a click on item `index` into a selection action.
pub fn handle_click(resp: &Response, index: usize, ctx: &mut ViewCtx<'_>) {
    let mods = resp.ctx.input(|i| i.modifiers);
    if resp.clicked() || resp.secondary_clicked() {
        ctx.actions.push(Action::SelectIndex {
            index,
            extend: mods.shift,
            toggle: mods.command || mods.ctrl,
        });
    }
}

/// Open on double click, or on single click when that preference is set.
pub fn handle_activate(resp: &Response, entry: &Entry, ctx: &mut ViewCtx<'_>) {
    let single = ctx.settings.single_click_open;
    let mods = resp.ctx.input(|i| i.modifiers);
    let plain = !(mods.shift || mods.ctrl || mods.command);
    if resp.double_clicked() || (single && resp.clicked() && plain) {
        ctx.actions.push(Action::Activate(entry.path.clone()));
    }
    if resp.middle_clicked() && entry.is_dir {
        ctx.actions.push(Action::OpenInNewTab(entry.path.clone()));
    }
}

/// Right-click menu for an item.
pub fn item_menu(resp: &Response, entry: &Entry, tab: &Tab, ctx: &mut ViewCtx<'_>) {
    let count = tab.selection.len().max(1);
    resp.context_menu(|ui| {
        ui.set_min_width(200.0);
        if ui.button("Open").clicked() {
            ctx.actions.push(Action::Activate(entry.path.clone()));
            ui.close();
        }
        if entry.is_dir && ui.button("Open in new tab").clicked() {
            ctx.actions.push(Action::OpenInNewTab(entry.path.clone()));
            ui.close();
        }
        if !entry.is_dir && ui.button("Show in containing folder").clicked() {
            ctx.actions.push(Action::RevealInOsFileManager);
            ui.close();
        }
        ui.separator();
        if ui.button("Cut").clicked() {
            ctx.actions.push(Action::Cut);
            ui.close();
        }
        if ui.button("Copy").clicked() {
            ctx.actions.push(Action::Copy);
            ui.close();
        }
        if entry.is_dir {
            let enabled = !ctx.clipboard.is_empty();
            if ui
                .add_enabled(enabled, egui::Button::new("Paste into"))
                .clicked()
            {
                ctx.actions.push(Action::Navigate(entry.path.clone()));
                ctx.actions.push(Action::Paste);
                ui.close();
            }
        }
        if ui.button("Copy path").clicked() {
            ctx.actions.push(Action::CopyPath);
            ui.close();
        }
        ui.separator();
        if ui.button("Rename").clicked() {
            ctx.actions.push(Action::BeginRename);
            ui.close();
        }
        let delete_label = if count > 1 {
            format!("Delete {count} items")
        } else {
            "Delete".to_string()
        };
        if ui.button(delete_label).clicked() {
            ctx.actions
                .push(Action::DeleteSelected { permanent: false });
            ui.close();
        }
        if ui.button("Delete permanently").clicked() {
            ctx.actions.push(Action::DeleteSelected { permanent: true });
            ui.close();
        }
        ui.separator();
        if entry.is_dir {
            if ui.button("Pin to sidebar").clicked() {
                ctx.actions.push(Action::Pin(entry.path.clone()));
                ui.close();
            }
            if ui.button("Open in terminal").clicked() {
                ctx.actions.push(Action::OpenTerminal);
                ui.close();
            }
        }
        if ui.button("Properties").clicked() {
            ctx.actions.push(Action::ShowProperties);
            ui.close();
        }
    });
}

/// Right-click menu for empty space in the listing.
pub fn background_menu(resp: &Response, ctx: &mut ViewCtx<'_>) {
    resp.context_menu(|ui| {
        ui.set_min_width(200.0);
        ui.menu_button("View", |ui| {
            for mode in ViewMode::ALL {
                if ui.button(mode.label()).clicked() {
                    ctx.actions.push(Action::SetViewMode(mode));
                    ui.close();
                }
            }
        });
        ui.menu_button("Sort by", |ui| {
            for key in explorer_core::SortKey::ALL {
                if ui.button(key.label()).clicked() {
                    ctx.actions.push(Action::SortBy(key));
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Folders first").clicked() {
                ctx.actions.push(Action::ToggleDirsFirst);
                ui.close();
            }
        });
        ui.separator();
        if ui.button("New folder").clicked() {
            ctx.actions.push(Action::NewFolder);
            ui.close();
        }
        if ui.button("New file").clicked() {
            ctx.actions.push(Action::NewFile);
            ui.close();
        }
        ui.separator();
        let enabled = !ctx.clipboard.is_empty();
        if ui
            .add_enabled(enabled, egui::Button::new("Paste"))
            .clicked()
        {
            ctx.actions.push(Action::Paste);
            ui.close();
        }
        if ui.button("Select all").clicked() {
            ctx.actions.push(Action::SelectAll);
            ui.close();
        }
        ui.separator();
        if ui.button("Open in terminal").clicked() {
            ctx.actions.push(Action::OpenTerminal);
            ui.close();
        }
        if ui.button("Refresh").clicked() {
            ctx.actions.push(Action::Refresh);
            ui.close();
        }
        if ui.button("Properties").clicked() {
            ctx.actions.push(Action::ShowProperties);
            ui.close();
        }
    });
}

/// Column text for an entry, shared by details and tiles.
pub fn cell_text(entry: &Entry, column: crate::action::Column) -> String {
    use crate::action::Column;
    match column {
        Column::Size => match entry.size {
            Some(b) => explorer_core::fmt::size(b),
            None => String::new(),
        },
        Column::Kind => entry.class.label().to_string(),
        Column::Modified => entry
            .modified
            .map(explorer_core::fmt::time)
            .unwrap_or_default(),
        Column::Created => entry
            .created
            .map(explorer_core::fmt::time)
            .unwrap_or_default(),
        Column::Extension => {
            if entry.is_dir {
                String::new()
            } else if entry.ext.is_empty() {
                "—".into()
            } else {
                format!(".{}", entry.ext)
            }
        }
    }
}

/// True when an entry should be dimmed because it is on the clipboard as a cut.
pub fn is_cut(entry: &Entry, clipboard: &Clipboard) -> bool {
    matches!(clipboard.mode, Some(explorer_core::ops::ClipMode::Cut))
        && clipboard.contains(&entry.path)
}
