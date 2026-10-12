//! Navigation pane: quick access, pinned folders, volumes and a folder tree.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use egui::{Sense, Ui, Vec2};
use explorer_core::{Places, Volume};

use crate::action::Action;
use crate::icons;
use crate::theme::Palette;

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
            section(ui, pal, "Quick access");
            for place in &places.quick {
                place_row(ui, &place.name, &place.path, current, pal, actions, false);
            }

            if !places.pinned.is_empty() {
                section(ui, pal, "Pinned");
                for place in &places.pinned {
                    place_row(ui, &place.name, &place.path, current, pal, actions, true);
                }
            }

            section(ui, pal, "This device");
            for vol in &places.volumes {
                volume_row(ui, vol, current, pal, actions);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let (rect, twisty) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::click());
                icons::chevron(
                    ui.painter(),
                    rect.center(),
                    8.0,
                    state.show_tree,
                    pal.text_dim,
                );
                let label = ui.add(
                    egui::Button::new(
                        egui::RichText::new("Folders")
                            .small()
                            .strong()
                            .color(pal.text_dim),
                    )
                    .frame(false),
                );
                if twisty.clicked() || label.clicked() {
                    state.show_tree = !state.show_tree;
                }
            });
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

fn section(ui: &mut Ui, pal: &Palette, title: &str) {
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(title)
            .small()
            .color(pal.text_dim)
            .strong(),
    );
}

fn place_row(
    ui: &mut Ui,
    name: &str,
    path: &Path,
    current: &Path,
    pal: &Palette,
    actions: &mut Vec<Action>,
    pinned: bool,
) {
    let selected = current == path;
    let resp = ui.selectable_label(selected, format!("  {name}"));
    let _ = pal;
    if resp.clicked() {
        actions.push(Action::Navigate(path.to_path_buf()));
    }
    if resp.middle_clicked() {
        actions.push(Action::OpenInNewTab(path.to_path_buf()));
    }
    resp.clone().on_hover_text(path.display().to_string());
    let owned = path.to_path_buf();
    resp.context_menu(|ui| {
        if ui.button("Open in new tab").clicked() {
            actions.push(Action::OpenInNewTab(owned.clone()));
            ui.close();
        }
        if pinned {
            if ui.button("Unpin").clicked() {
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
    let selected = current == vol.mount;
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
        icons::drive(ui.painter(), rect, vol.used_fraction(), pal);
        let resp = ui.selectable_label(selected, vol.name.as_str());
        if resp.clicked() {
            actions.push(Action::Navigate(vol.mount.clone()));
        }
        if resp.middle_clicked() {
            actions.push(Action::OpenInNewTab(vol.mount.clone()));
        }
        resp.on_hover_text(format!(
            "{}\n{} free of {}\nFilesystem: {}{}",
            vol.mount.display(),
            explorer_core::fmt::size(vol.available),
            explorer_core::fmt::size(vol.total),
            vol.fs,
            if vol.removable { "\nRemovable" } else { "" }
        ));
    });
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

    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 10.0);
        let (rect, twisty) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
        icons::chevron(ui.painter(), rect.center(), 9.0, open, pal.text_dim);
        if twisty.clicked() {
            if open {
                state.open.remove(path);
            } else {
                state.open.insert(path.to_path_buf());
            }
        }
        let resp = ui.selectable_label(selected, name);
        if resp.clicked() {
            actions.push(Action::Navigate(path.to_path_buf()));
            state.open.insert(path.to_path_buf());
        }
        if resp.middle_clicked() {
            actions.push(Action::OpenInNewTab(path.to_path_buf()));
        }
    });

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
