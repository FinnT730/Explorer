//! Modal dialogs: create, confirm delete, properties and error reporting.

use std::path::PathBuf;

use egui::{Context, Ui};

use crate::action::Action;
use crate::theme::Palette;

#[derive(Clone, Debug)]
pub enum Dialog {
    NewFolder {
        dir: PathBuf,
        name: String,
    },
    NewFile {
        dir: PathBuf,
        name: String,
    },
    Rename {
        path: PathBuf,
        name: String,
    },
    ConfirmDelete {
        paths: Vec<PathBuf>,
        permanent: bool,
    },
    Properties {
        paths: Vec<PathBuf>,
        /// Filled in by the background measurement, for folders.
        measured: Option<Measured>,
        request: Option<u64>,
    },
    Error {
        title: String,
        body: String,
    },
    Shortcuts,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Measured {
    pub bytes: u64,
    pub files: usize,
    pub dirs: usize,
}

/// What the dialog decided this frame.
pub enum DialogOutcome {
    None,
    Close,
    Submit(Action),
}

pub fn show(ctx: &Context, dialog: &mut Dialog, pal: &Palette) -> DialogOutcome {
    let mut outcome = DialogOutcome::None;
    let title = match dialog {
        Dialog::NewFolder { .. } => "New folder",
        Dialog::NewFile { .. } => "New file",
        Dialog::Rename { .. } => "Rename",
        Dialog::ConfirmDelete { permanent, .. } => {
            if *permanent {
                "Delete permanently"
            } else {
                "Delete"
            }
        }
        Dialog::Properties { .. } => "Properties",
        Dialog::Error { .. } => "Problem",
        Dialog::Shortcuts => "Keyboard shortcuts",
    };

    egui::Modal::new(egui::Id::new("explorer-dialog")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.heading(title);
        ui.add_space(6.0);
        match dialog {
            Dialog::NewFolder { dir, name } | Dialog::NewFile { dir, name } => {
                ui.label(
                    egui::RichText::new(format!("In {}", dir.display()))
                        .small()
                        .color(pal.text_dim),
                );
                let field_id = ui.id().with("name-field");
                let resp = ui.add(
                    egui::TextEdit::singleline(name)
                        .id(field_id)
                        .hint_text("Name")
                        .desired_width(f32::INFINITY),
                );
                if !resp.has_focus() {
                    resp.request_focus();
                    // Select the stem so typing replaces it but keeps ".txt".
                    let stem = crate::selectable_stem(name);
                    select_range(ui.ctx(), field_id, 0..stem);
                }
                // The modal owns the keyboard, so Enter here is unambiguous.
                let entered = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let valid = is_valid_name(name);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let pressed = ui.add_enabled(valid, egui::Button::new("Create")).clicked();
                    if valid && (pressed || entered) {
                        // The app builds the task from the dialog's own fields.
                        outcome = DialogOutcome::Submit(Action::Confirm);
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = DialogOutcome::Close;
                    }
                });
                if !valid && !name.trim().is_empty() {
                    ui.label(
                        egui::RichText::new("That name contains invalid characters")
                            .small()
                            .color(egui::Color32::from_rgb(0xd0, 0x4a, 0x42)),
                    );
                }
            }
            Dialog::Rename { path, name } => {
                ui.label(
                    egui::RichText::new(path.display().to_string())
                        .small()
                        .color(pal.text_dim),
                );
                let field_id = ui.id().with("rename-field");
                let resp = ui.add(
                    egui::TextEdit::singleline(name)
                        .id(field_id)
                        .desired_width(f32::INFINITY),
                );
                if !resp.has_focus() {
                    resp.request_focus();
                    let stem = crate::selectable_stem(name);
                    select_range(ui.ctx(), field_id, 0..stem);
                }
                let entered = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let valid = is_valid_name(name);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let pressed = ui.add_enabled(valid, egui::Button::new("Rename")).clicked();
                    if valid && (pressed || entered) {
                        outcome = DialogOutcome::Submit(Action::Confirm);
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = DialogOutcome::Close;
                    }
                });
            }
            Dialog::ConfirmDelete { paths, permanent } => {
                let msg = if *permanent {
                    format!(
                        "Permanently delete {} item{}? This cannot be undone.",
                        paths.len(),
                        if paths.len() == 1 { "" } else { "s" }
                    )
                } else {
                    format!(
                        "Move {} item{} to the recycle bin?",
                        paths.len(),
                        if paths.len() == 1 { "" } else { "s" }
                    )
                };
                ui.label(msg);
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(140.0)
                    .show(ui, |ui| {
                        for p in paths.iter().take(50) {
                            ui.label(
                                egui::RichText::new(p.display().to_string())
                                    .small()
                                    .color(pal.text_dim),
                            );
                        }
                        if paths.len() > 50 {
                            ui.label(
                                egui::RichText::new(format!("…and {} more", paths.len() - 50))
                                    .small()
                                    .color(pal.text_dim),
                            );
                        }
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        outcome = DialogOutcome::Submit(Action::Confirm);
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = DialogOutcome::Close;
                    }
                });
            }
            Dialog::Properties {
                paths,
                measured,
                request: _,
            } => {
                properties_body(ui, paths, *measured, pal);
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    outcome = DialogOutcome::Close;
                }
            }
            Dialog::Error { title, body } => {
                ui.label(egui::RichText::new(title.clone()).strong());
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.label(body.clone());
                    });
                ui.add_space(8.0);
                if ui.button("OK").clicked() {
                    outcome = DialogOutcome::Close;
                }
            }
            Dialog::Shortcuts => {
                shortcuts_body(ui, pal);
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    outcome = DialogOutcome::Close;
                }
            }
        }
    });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        outcome = DialogOutcome::Close;
    }
    outcome
}

fn properties_body(ui: &mut Ui, paths: &[PathBuf], measured: Option<Measured>, pal: &Palette) {
    if paths.len() == 1 {
        let path = &paths[0];
        let entry = explorer_core::Entry::from_path(path.clone()).ok();
        ui.label(egui::RichText::new(explorer_core::entry::file_name_of(path)).strong());
        ui.label(
            egui::RichText::new(path.display().to_string())
                .small()
                .color(pal.text_dim),
        );
        ui.separator();
        if let Some(e) = entry {
            kv(ui, pal, "Kind", e.class.label().to_string());
            match e.size {
                Some(b) => {
                    kv(ui, pal, "Size", explorer_core::fmt::size(b));
                    kv(ui, pal, "On disk", explorer_core::fmt::size_exact(b));
                }
                None => match measured {
                    Some(m) => {
                        kv(ui, pal, "Size", explorer_core::fmt::size(m.bytes));
                        kv(ui, pal, "Exact", explorer_core::fmt::size_exact(m.bytes));
                        kv(
                            ui,
                            pal,
                            "Contains",
                            format!("{} files, {} folders", m.files, m.dirs),
                        );
                    }
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new("Measuring folder…")
                                    .small()
                                    .color(pal.text_dim),
                            );
                        });
                    }
                },
            }
            if let Some(t) = e.created {
                kv(ui, pal, "Created", explorer_core::fmt::time_long(t));
            }
            if let Some(t) = e.modified {
                kv(ui, pal, "Modified", explorer_core::fmt::time_long(t));
            }
            if let Some(t) = e.accessed {
                kv(ui, pal, "Accessed", explorer_core::fmt::time_long(t));
            }
            kv(
                ui,
                pal,
                "Attributes",
                [
                    e.readonly.then_some("read-only"),
                    e.hidden.then_some("hidden"),
                    e.is_symlink.then_some("symlink"),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", "),
            );
        }
    } else {
        kv(ui, pal, "Items", paths.len().to_string());
        let total: u64 = paths
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .filter(|m| m.is_file())
            .map(|m| m.len())
            .sum();
        kv(ui, pal, "Size of files", explorer_core::fmt::size(total));
        if let Some(m) = measured {
            kv(
                ui,
                pal,
                "Including folders",
                explorer_core::fmt::size(m.bytes),
            );
        }
    }
}

fn kv(ui: &mut Ui, pal: &Palette, k: &str, v: String) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [120.0, 18.0],
            egui::Label::new(egui::RichText::new(k).color(pal.text_dim)),
        );
        ui.label(v);
    });
}

fn shortcuts_body(ui: &mut Ui, pal: &Palette) {
    const ROWS: &[(&str, &str)] = &[
        ("Enter / double-click", "Open"),
        ("Alt+Left / Backspace", "Back"),
        ("Alt+Right", "Forward"),
        ("Alt+Up", "Up one folder"),
        ("F5 / Ctrl+R", "Refresh"),
        ("Ctrl+T", "New tab"),
        ("Ctrl+W", "Close tab"),
        ("Ctrl+Tab", "Next tab"),
        ("Ctrl+1 / 2 / 3 / 4", "Details / List / Icons / Tiles"),
        ("Ctrl+F", "Search this folder"),
        ("Ctrl+H", "Toggle hidden items"),
        ("Ctrl+A", "Select all"),
        ("Ctrl+C / Ctrl+X / Ctrl+V", "Copy / cut / paste"),
        ("F2", "Rename"),
        ("Delete", "Move to recycle bin"),
        ("Shift+Delete", "Delete permanently"),
        ("Ctrl+Shift+N", "New folder"),
        ("Ctrl+L", "Edit the path"),
        ("Ctrl+P", "Toggle preview pane"),
        ("Ctrl+B", "Toggle navigation pane"),
        ("Alt+Enter", "Properties"),
        ("+ / -", "Grow / shrink icons"),
        ("F1", "This list"),
    ];
    egui::ScrollArea::vertical()
        .max_height(360.0)
        .show(ui, |ui| {
            for (keys, what) in ROWS {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [190.0, 18.0],
                        egui::Label::new(egui::RichText::new(*keys).monospace().small()),
                    );
                    ui.label(egui::RichText::new(*what).color(pal.text_dim));
                });
            }
        });
}

/// Pre-select a range in a text field, so typing replaces the suggested name
/// the way it does in Explorer's rename box.
pub fn select_range(ctx: &Context, id: egui::Id, chars: std::ops::Range<usize>) {
    if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(chars.start),
                egui::text::CCursor::new(chars.end),
            )));
        egui::TextEdit::store_state(ctx, id, state);
    }
}

/// Reject path separators and the characters Windows forbids in names.
pub fn is_valid_name(name: &str) -> bool {
    let n = name.trim();
    !n.is_empty()
        && n != "."
        && n != ".."
        && !n.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
}
