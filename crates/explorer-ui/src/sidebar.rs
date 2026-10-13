//! Navigation pane: quick access, pinned folders, volumes and a folder tree.
//!
//! Rows follow the Fluent nav-view pattern: 30 px tall, rounded hover, and a
//! tinted fill with an accent bar for the current location.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Align2, CornerRadius, FontId, Rect, Response, Sense, Ui, Vec2};
use explorer_core::{FileClass, Places, Volume};

use crate::action::Action;
use crate::icons;
use crate::theme::{Palette, RADIUS_SMALL};

const ROW_H: f32 = 30.0;
const INDENT: f32 = 14.0;

pub struct SidebarState {
    /// Expanded nodes in the tree.
    pub open: HashSet<PathBuf>,
    pub show_tree: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            open: HashSet::new(),
            show_tree: true,
        }
    }
}

pub fn show(
    ui: &mut Ui,
    places: &Places,
    current: &Path,
    state: &mut SidebarState,
    pal: &Palette,
    actions: &mut Vec<Action>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;

            header(ui, pal, "Quick access");
            for place in &places.quick {
                let row = nav_row(
                    ui,
                    pal,
                    0,
                    &place.name,
                    Glyph::Folder,
                    current == place.path,
                );
                wire(&row, &place.path, actions, pal, false);
            }

            if !places.pinned.is_empty() {
                header(ui, pal, "Pinned");
                for place in &places.pinned {
                    let row = nav_row(ui, pal, 0, &place.name, Glyph::Pin, current == place.path);
                    wire(&row, &place.path, actions, pal, true);
                }
            }

            header(ui, pal, "This device");
            for vol in &places.volumes {
                volume_row(ui, vol, current, pal, actions);
            }

            ui.add_space(6.0);
            let twisty = section_toggle(ui, pal, "Folders", state.show_tree);
            if twisty.clicked() {
                state.show_tree = !state.show_tree;
            }
            if state.show_tree {
                let roots: Vec<PathBuf> = if places.volumes.is_empty() {
                    vec![PathBuf::from("/")]
                } else {
                    places.volumes.iter().map(|v| v.mount.clone()).collect()
                };
                for root in roots {
                    tree_node(ui, &root, current, state, pal, actions, 0);
                }
            }
            ui.add_space(8.0);
        });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Glyph {
    Folder,
    Pin,
    Drive(u8),
    Tree(bool),
}

fn header(ui: &mut Ui, pal: &Palette, title: &str) {
    ui.add_space(8.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        pal.text_dim,
    );
    ui.add_space(2.0);
}

fn section_toggle(ui: &mut Ui, pal: &Palette, title: &str, open: bool) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 22.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS_SMALL), pal.hover);
    }
    icons::chevron(
        ui.painter(),
        egui::pos2(rect.left() + 12.0, rect.center().y),
        9.0,
        open,
        pal.text_dim,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 24.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(11.0),
        pal.text_dim,
    );
    resp
}

/// One navigation row. Returns its response so the caller can wire clicks.
fn nav_row(
    ui: &mut Ui,
    pal: &Palette,
    depth: usize,
    label: &str,
    glyph: Glyph,
    selected: bool,
) -> Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter();
    let radius = CornerRadius::same(RADIUS_SMALL);

    if selected {
        painter.rect_filled(rect, radius, pal.selection);
        painter.rect_filled(
            Rect::from_min_size(
                rect.min + Vec2::new(0.0, rect.height() * 0.25),
                Vec2::new(3.0, rect.height() * 0.5),
            ),
            CornerRadius::same(2),
            pal.accent,
        );
    } else if resp.hovered() {
        painter.rect_filled(rect, radius, pal.hover);
    }

    let x = rect.left() + 8.0 + depth as f32 * INDENT;
    let icon = Rect::from_min_size(egui::pos2(x, rect.center().y - 8.0), Vec2::splat(16.0));
    match glyph {
        Glyph::Folder => icons::draw(painter, icon, FileClass::Folder, "", pal, false),
        Glyph::Pin => {
            icons::draw(painter, icon, FileClass::Folder, "", pal, false);
            painter.circle_filled(icon.right_top() + Vec2::new(-1.0, 2.0), 2.5, pal.accent);
        }
        Glyph::Drive(used) => icons::drive(painter, icon, used as f32 / 255.0, pal),
        Glyph::Tree(open) => {
            icons::chevron(
                painter,
                egui::pos2(icon.center().x - 2.0, icon.center().y),
                9.0,
                open,
                pal.text_dim,
            );
        }
    }

    painter.text(
        egui::pos2(icon.right() + 8.0, rect.center().y),
        Align2::LEFT_CENTER,
        crate::views::list::elide(label, rect.width() - (icon.right() - rect.left()) - 20.0),
        FontId::proportional(12.5),
        if selected {
            pal.selection_text
        } else {
            pal.text
        },
    );
    resp
}

fn wire(resp: &Response, path: &Path, actions: &mut Vec<Action>, _pal: &Palette, pinned: bool) {
    if resp.clicked() {
        actions.push(Action::Navigate(path.to_path_buf()));
    }
    if resp.middle_clicked() {
        actions.push(Action::OpenInNewTab(path.to_path_buf()));
    }
    let owned = path.to_path_buf();
    resp.clone().on_hover_text(path.display().to_string());
    resp.context_menu(|ui| {
        if ui.button("Open").clicked() {
            actions.push(Action::Navigate(owned.clone()));
            ui.close();
        }
        if ui.button("Open in new tab").clicked() {
            actions.push(Action::OpenInNewTab(owned.clone()));
            ui.close();
        }
        ui.separator();
        if pinned {
            if ui.button("Unpin from sidebar").clicked() {
                actions.push(Action::Unpin(owned.clone()));
                ui.close();
            }
        } else if ui.button("Pin to sidebar").clicked() {
            actions.push(Action::Pin(owned.clone()));
            ui.close();
        }
    });
}

fn volume_row(ui: &mut Ui, vol: &Volume, current: &Path, pal: &Palette, actions: &mut Vec<Action>) {
    let used = (vol.used_fraction() * 255.0) as u8;
    let row = nav_row(
        ui,
        pal,
        0,
        &vol.name,
        Glyph::Drive(used),
        current == vol.mount,
    );
    if row.clicked() {
        actions.push(Action::Navigate(vol.mount.clone()));
    }
    if row.middle_clicked() {
        actions.push(Action::OpenInNewTab(vol.mount.clone()));
    }
    row.on_hover_text(format!(
        "{}\n{} free of {}\nFilesystem: {}{}",
        vol.mount.display(),
        explorer_core::fmt::size(vol.available),
        explorer_core::fmt::size(vol.total),
        vol.fs,
        if vol.removable { "\nRemovable" } else { "" }
    ));
}

fn tree_node(
    ui: &mut Ui,
    path: &Path,
    current: &Path,
    state: &mut SidebarState,
    pal: &Palette,
    actions: &mut Vec<Action>,
    depth: usize,
) {
    if depth > 16 {
        return;
    }
    let name = {
        let n = explorer_core::entry::file_name_of(path);
        if n.is_empty() {
            path.display().to_string()
        } else {
            n
        }
    };
    let open = state.open.contains(path);
    let selected = current == path;

    let row = nav_row(ui, pal, depth, &name, Glyph::Tree(open), selected);
    // The left third of the row is the twisty; the rest navigates.
    let twisty_hit = row
        .interact_pointer_pos()
        .map(|p| p.x < row.rect.left() + 10.0 + depth as f32 * INDENT + 20.0)
        .unwrap_or(false);
    if row.clicked() {
        if twisty_hit {
            if open {
                state.open.remove(path);
            } else {
                state.open.insert(path.to_path_buf());
            }
        } else {
            actions.push(Action::Navigate(path.to_path_buf()));
            state.open.insert(path.to_path_buf());
        }
    }
    if row.double_clicked() || row.middle_clicked() {
        actions.push(Action::OpenInNewTab(path.to_path_buf()));
    }

    if !open {
        return;
    }
    // Children are read lazily, only for expanded nodes.
    let mut children: Vec<PathBuf> = std::fs::read_dir(path)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    children.sort_by(|a, b| {
        explorer_core::natural_cmp(
            &explorer_core::entry::file_name_of(a).to_lowercase(),
            &explorer_core::entry::file_name_of(b).to_lowercase(),
        )
    });
    for child in children.into_iter().take(500) {
        tree_node(ui, &child, current, state, pal, actions, depth + 1);
    }
}
