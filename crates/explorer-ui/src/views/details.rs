//! Details view: a sortable, resizable, virtualised table.

use egui::{Align, Layout, Sense, Ui};
use egui_extras::{Column as TableColumn, TableBuilder};

use crate::action::{Action, Column};
use crate::tab::Tab;

use super::{
    cell_text, draw_icon, handle_activate, handle_click, is_cut, item_menu, text_color, ViewCtx,
};

const ROW_H: f32 = 30.0;

pub fn show(ui: &mut Ui, tab: &mut Tab, ctx: &mut ViewCtx<'_>) {
    let cols = ctx.settings.columns;
    let mut active: Vec<Column> = Vec::with_capacity(5);
    if cols.size {
        active.push(Column::Size);
    }
    if cols.kind {
        active.push(Column::Kind);
    }
    if cols.extension {
        active.push(Column::Extension);
    }
    if cols.modified {
        active.push(Column::Modified);
    }
    if cols.created {
        active.push(Column::Created);
    }

    let mut builder = TableBuilder::new(ui)
        .striped(false)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .min_scrolled_height(0.0)
        .auto_shrink([false, false])
        .column(TableColumn::remainder().at_least(160.0).clip(true));
    for col in &active {
        let width = match col {
            Column::Size => 90.0,
            Column::Kind => 120.0,
            Column::Extension => 70.0,
            Column::Modified | Column::Created => 140.0,
        };
        builder = builder.column(TableColumn::initial(width).at_least(60.0).clip(true));
    }

    let sort = tab.sort;
    let mut pending_sort: Option<explorer_core::SortKey> = None;
    let table = builder.header(22.0, |mut header| {
        header.col(|ui| {
            if header_button(ui, "Name", sort, explorer_core::SortKey::Name) {
                pending_sort = Some(explorer_core::SortKey::Name);
            }
        });
        for col in &active {
            header.col(|ui| {
                let key = col.sort_key();
                if header_button(ui, col.label(), sort, key) {
                    pending_sort = Some(key);
                }
            });
        }
    });

    let total = tab.view.len();
    let mut clicked: Option<(usize, egui::Response)> = None;
    let scroll_target = tab.scroll_to_cursor.then_some(tab.cursor).flatten();
    // Inline rename is edited in a local buffer so the row closures can keep
    // borrowing the tab immutably; it is written back after the table.
    let mut rename = tab.renaming.clone();

    table.body(|body| {
        body.rows(ROW_H, total, |mut row| {
            let index = row.index();
            let Some(entry) = tab.entry_at(index) else {
                return;
            };
            let selected = tab.selection.contains(&entry.path);
            let cut = is_cut(entry, ctx.clipboard);
            row.set_selected(selected);

            row.col(|ui| {
                let h = ui.available_height();
                let (icon_rect, _) = ui.allocate_exact_size(egui::vec2(h, h), Sense::hover());
                draw_icon(ui, icon_rect.shrink(2.0), entry, ctx);
                ui.add_space(4.0);

                if let Some((path, buf)) = rename.as_mut() {
                    if *path == entry.path {
                        let field_id = ui.id().with(("rename", index));
                        let edit = egui::TextEdit::singleline(buf)
                            .id(field_id)
                            .desired_width(ui.available_width())
                            .margin(egui::vec2(2.0, 0.0));
                        let r = ui.add(edit);
                        if !r.has_focus() {
                            r.request_focus();
                            let stem = crate::selectable_stem(buf);
                            crate::dialogs::select_range(ui.ctx(), field_id, 0..stem);
                        }
                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if enter {
                            ctx.actions.push(Action::CommitRename {
                                from: path.clone(),
                                to_name: buf.clone(),
                            });
                        } else if escape || (r.lost_focus() && !r.has_focus()) {
                            ctx.actions.push(Action::CancelRename);
                        }
                        return;
                    }
                }

                let mut text =
                    egui::RichText::new(&entry.name).color(text_color(selected, ctx.pal, ui));
                if cut {
                    text = text.weak();
                }
                ui.label(text);
            });

            for col in &active {
                row.col(|ui| {
                    let mut text = egui::RichText::new(cell_text(entry, *col)).color(if selected {
                        ctx.pal.selection_text
                    } else {
                        ctx.pal.text_dim
                    });
                    if cut {
                        text = text.weak();
                    }
                    ui.label(text);
                });
            }

            let resp = row.response();
            if Some(index) == scroll_target {
                resp.scroll_to_me(Some(Align::Center));
            }
            // The menu has to be re-registered every frame or the popup closes
            // the moment the click frame ends.
            item_menu(&resp, entry, tab, ctx);
            if clicked.is_none()
                && (resp.clicked()
                    || resp.secondary_clicked()
                    || resp.double_clicked()
                    || resp.middle_clicked())
            {
                clicked = Some((index, resp));
            }
        });
    });

    tab.renaming = rename;
    if let Some(key) = pending_sort {
        ctx.actions.push(Action::SortBy(key));
    }
    if let Some((index, resp)) = clicked {
        if let Some(entry) = tab.entry_at(index).cloned() {
            handle_click(&resp, index, ctx);
            handle_activate(&resp, &entry, ctx);
        }
    }
    tab.scroll_to_cursor = false;
}

/// A column header: a frameless button, plus a painted sort triangle when
/// this is the active sort column.
fn header_button(
    ui: &mut Ui,
    text: &str,
    sort: explorer_core::SortSpec,
    key: explorer_core::SortKey,
) -> bool {
    let resp = ui.add(
        egui::Button::new(egui::RichText::new(text).strong())
            .frame(false)
            .min_size(egui::vec2(0.0, 20.0)),
    );
    if sort.key == key {
        let c = egui::pos2(resp.rect.right() + 7.0, resp.rect.center().y);
        crate::icons::triangle(
            ui.painter(),
            c,
            9.0,
            sort.dir == explorer_core::SortDir::Asc,
            ui.visuals().strong_text_color(),
        );
    }
    resp.clicked()
}
