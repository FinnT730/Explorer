//! List view: narrow single-line rows laid out in wrapping columns, the way
//! Explorer's "List" mode works.

use egui::{Align2, FontId, Sense, Ui, Vec2};

use crate::tab::Tab;

use super::{
    draw_icon, handle_activate, handle_click, is_cut, item_menu, paint_row_background, text_color,
    ViewCtx,
};

const ROW_H: f32 = 22.0;
const COL_W: f32 = 240.0;

pub fn show(ui: &mut Ui, tab: &mut Tab, ctx: &mut ViewCtx<'_>) {
    let avail = ui.available_size();
    let rows_per_col = ((avail.y / ROW_H).floor() as usize).max(1);
    let total = tab.view.len();
    let col_count = total.div_ceil(rows_per_col);

    egui::ScrollArea::horizontal()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let size = Vec2::new(COL_W * col_count as f32, rows_per_col as f32 * ROW_H);
            let (rect, _bg) = ui.allocate_exact_size(size, Sense::hover());
            let mut hit: Option<(usize, egui::Response)> = None;

            for index in 0..total {
                let col = index / rows_per_col;
                let row = index % rows_per_col;
                let item_rect = egui::Rect::from_min_size(
                    rect.min + Vec2::new(col as f32 * COL_W, row as f32 * ROW_H),
                    Vec2::new(COL_W - 6.0, ROW_H),
                );
                if !ui.is_rect_visible(item_rect) {
                    continue;
                }
                let Some(entry) = tab.entry_at(index) else {
                    continue;
                };
                let selected = tab.selection.contains(&entry.path);
                let resp = ui.interact(
                    item_rect,
                    ui.id().with(("list-item", index)),
                    Sense::click(),
                );
                paint_row_background(
                    ui,
                    item_rect,
                    selected,
                    tab.cursor == Some(index),
                    resp.hovered(),
                    ctx.pal,
                );
                let icon = egui::Rect::from_min_size(
                    item_rect.min + Vec2::new(3.0, 3.0),
                    Vec2::splat(ROW_H - 6.0),
                );
                draw_icon(ui, icon, entry, ctx);
                let color = if is_cut(entry, ctx.clipboard) {
                    ctx.pal.text_dim
                } else {
                    text_color(selected, ctx.pal, ui)
                };
                ui.painter().text(
                    egui::pos2(icon.right() + 6.0, item_rect.center().y),
                    Align2::LEFT_CENTER,
                    elide(&entry.name, COL_W - ROW_H - 16.0),
                    FontId::proportional(13.0),
                    color,
                );
                item_menu(&resp, entry, tab, ctx);
                if resp.clicked()
                    || resp.secondary_clicked()
                    || resp.double_clicked()
                    || resp.middle_clicked()
                {
                    hit = Some((index, resp));
                }
            }

            if let Some((index, resp)) = hit {
                if let Some(entry) = tab.entry_at(index).cloned() {
                    handle_click(&resp, index, ctx);
                    handle_activate(&resp, &entry, ctx);
                }
            }
        });
}

/// Trim a name to roughly fit `width`, with an ellipsis.
pub fn elide(name: &str, width: f32) -> String {
    let max_chars = (width / 6.5).floor().max(4.0) as usize;
    if name.chars().count() <= max_chars {
        return name.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let head: String = name.chars().take(keep).collect();
    format!("{head}\u{2026}")
}
