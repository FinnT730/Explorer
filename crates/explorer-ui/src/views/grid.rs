//! Icons and Tiles views: a virtualised grid of thumbnails.
//!
//! Only the visible rows are laid out and only their thumbnails are requested,
//! so a folder with 100k images scrolls at frame rate and decodes just what is
//! on screen.

use egui::{Align2, FontId, Sense, Ui, Vec2};

use crate::action::Column;
use crate::tab::Tab;

use super::{
    cell_text, draw_icon, handle_activate, handle_click, is_cut, item_menu, paint_row_background,
    text_color, ViewCtx,
};

pub fn show(ui: &mut Ui, tab: &mut Tab, ctx: &mut ViewCtx<'_>, tiles: bool) {
    let icon = tab.icon_size.clamp(32.0, 256.0);
    let label_h = 34.0;
    let cell = if tiles {
        Vec2::new(icon * 2.6 + 24.0, icon + 16.0)
    } else {
        Vec2::new(icon + 20.0, icon + label_h)
    };
    let spacing = 6.0;
    let avail_w = ui.available_width();
    let per_row = (((avail_w + spacing) / (cell.x + spacing)).floor() as usize).max(1);
    let total = tab.view.len();
    let rows = total.div_ceil(per_row);
    let row_h = cell.y + spacing;

    let cursor_row = tab
        .scroll_to_cursor
        .then_some(tab.cursor)
        .flatten()
        .map(|c| c / per_row);

    let out = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, cell.y, rows, |ui, range| {
            let mut hit: Option<(usize, egui::Response)> = None;

            for row in range.clone() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;
                    for col in 0..per_row {
                        let index = row * per_row + col;
                        if index >= total {
                            break;
                        }
                        let Some(entry) = tab.entry_at(index) else {
                            continue;
                        };
                        let selected = tab.selection.contains(&entry.path);
                        let (rect, resp) = ui.allocate_exact_size(cell, Sense::click());
                        paint_row_background(
                            ui,
                            rect,
                            selected,
                            tab.cursor == Some(index),
                            resp.hovered(),
                            ctx.pal,
                        );

                        let cut = is_cut(entry, ctx.clipboard);
                        let color = if cut {
                            ctx.pal.text_dim
                        } else {
                            text_color(selected, ctx.pal, ui)
                        };

                        if tiles {
                            let icon_rect = egui::Rect::from_min_size(
                                rect.min + Vec2::new(8.0, 8.0),
                                Vec2::splat(icon),
                            );
                            draw_icon(ui, icon_rect, entry, ctx);
                            let text_x = icon_rect.right() + 10.0;
                            let width = rect.right() - text_x - 8.0;
                            ui.painter().text(
                                egui::pos2(text_x, rect.top() + icon * 0.32),
                                Align2::LEFT_CENTER,
                                super::list::elide(&entry.name, width),
                                FontId::proportional(13.0),
                                color,
                            );
                            let meta = if entry.is_dir {
                                entry.class.label().to_string()
                            } else {
                                format!(
                                    "{} · {}",
                                    cell_text(entry, Column::Kind),
                                    cell_text(entry, Column::Size)
                                )
                            };
                            ui.painter().text(
                                egui::pos2(text_x, rect.top() + icon * 0.62),
                                Align2::LEFT_CENTER,
                                super::list::elide(&meta, width),
                                FontId::proportional(11.0),
                                ctx.pal.text_dim,
                            );
                        } else {
                            let icon_rect = egui::Rect::from_min_size(
                                egui::pos2(rect.center().x - icon / 2.0, rect.top() + 6.0),
                                Vec2::splat(icon),
                            );
                            draw_icon(ui, icon_rect, entry, ctx);
                            // Two lines of name under the icon, wrapped by hand
                            // so the grid metrics stay exact.
                            let (l1, l2) = wrap_two(&entry.name, rect.width() - 8.0);
                            let base = icon_rect.bottom() + 12.0;
                            ui.painter().text(
                                egui::pos2(rect.center().x, base),
                                Align2::CENTER_CENTER,
                                l1,
                                FontId::proportional(12.0),
                                color,
                            );
                            if let Some(l2) = l2 {
                                ui.painter().text(
                                    egui::pos2(rect.center().x, base + 14.0),
                                    Align2::CENTER_CENTER,
                                    l2,
                                    FontId::proportional(12.0),
                                    color,
                                );
                            }
                        }

                        if resp.hovered() {
                            resp.clone().on_hover_text(hover_text(entry));
                        }
                        item_menu(&resp, entry, tab, ctx);
                        if resp.clicked()
                            || resp.secondary_clicked()
                            || resp.double_clicked()
                            || resp.middle_clicked()
                        {
                            hit = Some((index, resp));
                        }
                    }
                });
            }

            if let Some((index, resp)) = hit {
                if let Some(entry) = tab.entry_at(index).cloned() {
                    handle_click(&resp, index, ctx);
                    handle_activate(&resp, &entry, ctx);
                }
            }
            range
        });

    // Keyboard navigation scrolls the grid by whole rows.
    if let Some(target) = cursor_row {
        if !out.inner.contains(&target) {
            let y = target as f32 * row_h;
            ui.scroll_to_rect(
                egui::Rect::from_min_size(egui::pos2(0.0, y), Vec2::new(1.0, row_h)),
                Some(egui::Align::Center),
            );
        }
    }
    tab.scroll_to_cursor = false;
}

fn hover_text(entry: &explorer_core::Entry) -> String {
    let mut s = entry.name.clone();
    s.push('\n');
    s.push_str(entry.class.label());
    if let Some(b) = entry.size {
        s.push_str(&format!("\n{}", explorer_core::fmt::size(b)));
    }
    if let Some(t) = entry.modified {
        s.push_str(&format!("\nModified {}", explorer_core::fmt::time(t)));
    }
    s
}

/// Split a name over at most two centred lines, eliding the overflow.
fn wrap_two(name: &str, width: f32) -> (String, Option<String>) {
    let per_line = (width / 6.2).floor().max(4.0) as usize;
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= per_line {
        return (name.to_string(), None);
    }
    let first: String = chars[..per_line].iter().collect();
    let rest = &chars[per_line..];
    if rest.len() <= per_line {
        (first, Some(rest.iter().collect()))
    } else {
        let second: String = rest[..per_line.saturating_sub(1)].iter().collect();
        (first, Some(format!("{second}\u{2026}")))
    }
}
