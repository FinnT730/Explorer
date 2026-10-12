//! Tab strip, navigation bar, breadcrumbs and the view controls.

use std::path::{Path, PathBuf};

use egui::{Align, Layout, Ui};
use explorer_core::{SortDir, SortKey};

use crate::action::{Action, Column};
use crate::settings::{Settings, Theme, ViewMode};
use crate::tab::Tab;
use crate::theme::Palette;

pub struct Bars<'a> {
    pub settings: &'a Settings,
    pub pal: &'a Palette,
    pub actions: &'a mut Vec<Action>,
    /// Buffer backing the editable path field, owned by the app.
    pub path_edit: &'a mut Option<String>,
    pub search: &'a mut String,
    pub focus_search: bool,
}

pub fn tab_strip(ui: &mut Ui, tabs: &[Tab], active: usize, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, tab) in tabs.iter().enumerate() {
            let selected = i == active;
            let title = tab.title();
            let label = crate::views::list::elide(&title, 150.0);
            let resp = ui.selectable_label(selected, format!(" {label} "));
            if resp.clicked() {
                bars.actions.push(Action::SelectTab(i));
            }
            if resp.middle_clicked() {
                bars.actions.push(Action::CloseTab(i));
            }
            resp.clone().on_hover_text(tab.path().display().to_string());
            resp.context_menu(|ui| {
                if ui.button("Duplicate tab").clicked() {
                    bars.actions.push(Action::DuplicateTab(i));
                    ui.close();
                }
                if ui.button("Close tab").clicked() {
                    bars.actions.push(Action::CloseTab(i));
                    ui.close();
                }
            });
            if tabs.len() > 1 {
                let close = ui.small_button("\u{00d7}");
                if close.clicked() {
                    bars.actions.push(Action::CloseTab(i));
                }
            }
            ui.add_space(6.0);
        }
        if ui
            .small_button("+")
            .on_hover_text("New tab (Ctrl+T)")
            .clicked()
        {
            bars.actions.push(Action::NewTab(None));
        }
    });
}

pub fn nav_bar(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        if icon_button(ui, crate::icons::Dir::Left, tab.history.can_go_back())
            .on_hover_text("Back (Alt+Left)")
            .clicked()
        {
            bars.actions.push(Action::Back);
        }
        if icon_button(ui, crate::icons::Dir::Right, tab.history.can_go_forward())
            .on_hover_text("Forward (Alt+Right)")
            .clicked()
        {
            bars.actions.push(Action::Forward);
        }
        if icon_button(ui, crate::icons::Dir::Up, tab.history.can_go_up())
            .on_hover_text("Up (Alt+Up)")
            .clicked()
        {
            bars.actions.push(Action::Up);
        }
        if icon_button(ui, crate::icons::Dir::Refresh, true)
            .on_hover_text("Refresh (F5)")
            .clicked()
        {
            bars.actions.push(Action::Refresh);
        }

        ui.separator();
        breadcrumbs(ui, tab.path(), bars);

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let hint = if tab.filter.is_searching() {
                "Searching…"
            } else {
                "Search this folder"
            };
            let edit = egui::TextEdit::singleline(bars.search)
                .hint_text(hint)
                .desired_width(180.0);
            let resp = ui.add(edit);
            if bars.focus_search {
                resp.request_focus();
            }
            if resp.changed() {
                bars.actions.push(Action::SetQuery(bars.search.clone()));
            }
            if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                bars.actions.push(Action::SearchHere);
            }
            if !bars.search.is_empty() && ui.small_button("\u{00d7}").clicked() {
                bars.actions.push(Action::ClearSearch);
            }
            if tab.is_loading() {
                ui.spinner();
            }
        });
    });
}

fn breadcrumbs(ui: &mut Ui, path: &Path, bars: &mut Bars<'_>) {
    // Click the trailing space to type a path, like the Explorer address bar.
    if let Some(buf) = bars.path_edit.as_mut() {
        let resp = ui.add(
            egui::TextEdit::singleline(buf)
                .desired_width(ui.available_width() - 200.0)
                .hint_text("Type a path and press Enter"),
        );
        resp.request_focus();
        if resp.lost_focus() {
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let p = PathBuf::from(shellexpand(buf));
                bars.actions.push(Action::Navigate(p));
            }
            *bars.path_edit = None;
        }
        return;
    }

    let mut acc = PathBuf::new();
    let segments: Vec<_> = path.components().collect();
    ui.spacing_mut().item_spacing.x = 1.0;
    for (i, comp) in segments.iter().enumerate() {
        acc.push(comp.as_os_str());
        let name = match comp {
            std::path::Component::RootDir => "/".to_string(),
            other => other.as_os_str().to_string_lossy().into_owned(),
        };
        if i > 0 {
            ui.label(egui::RichText::new("\u{203a}").color(bars.pal.text_dim));
        }
        let btn = ui.add(egui::Button::new(name).frame(false));
        if btn.clicked() {
            bars.actions.push(Action::Navigate(acc.clone()));
        }
        if btn.middle_clicked() {
            bars.actions.push(Action::OpenInNewTab(acc.clone()));
        }
        // Drop-down of the segment's subfolders, so you can hop sideways.
        let target = acc.clone();
        btn.context_menu(|ui| {
            ui.set_max_height(380.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut any = false;
                if let Ok(rd) = std::fs::read_dir(&target) {
                    let mut dirs: Vec<PathBuf> = rd
                        .filter_map(Result::ok)
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.path())
                        .collect();
                    dirs.sort();
                    for d in dirs.into_iter().take(200) {
                        any = true;
                        let label = explorer_core::entry::file_name_of(&d);
                        if ui.button(label).clicked() {
                            bars.actions.push(Action::Navigate(d));
                            ui.close();
                        }
                    }
                }
                if !any {
                    ui.label("No subfolders");
                }
            });
        });
    }

    let rest = ui.available_width() - 200.0;
    if rest > 10.0 {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(rest, 20.0), egui::Sense::click());
        let _ = rect;
        if resp.clicked() {
            *bars.path_edit = Some(path.display().to_string());
        }
        resp.on_hover_text("Click to edit the path");
    }
}

pub fn view_bar(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        ui.menu_button("View", |ui| {
            for mode in ViewMode::ALL {
                if ui
                    .selectable_label(tab.view_mode == mode, mode.label())
                    .clicked()
                {
                    bars.actions.push(Action::SetViewMode(mode));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .selectable_label(bars.settings.show_sidebar, "Navigation pane")
                .clicked()
            {
                bars.actions.push(Action::ToggleSidebar);
                ui.close();
            }
            if ui
                .selectable_label(bars.settings.show_preview, "Preview pane")
                .clicked()
            {
                bars.actions.push(Action::TogglePreview);
                ui.close();
            }
            if ui
                .selectable_label(bars.settings.show_status_bar, "Status bar")
                .clicked()
            {
                bars.actions.push(Action::ToggleStatusBar);
                ui.close();
            }
            ui.separator();
            if ui
                .selectable_label(bars.settings.show_hidden, "Hidden items")
                .clicked()
            {
                bars.actions.push(Action::ToggleHidden);
                ui.close();
            }
            ui.menu_button("Columns", |ui| {
                let c = bars.settings.columns;
                let on = |col: Column| match col {
                    Column::Size => c.size,
                    Column::Kind => c.kind,
                    Column::Modified => c.modified,
                    Column::Created => c.created,
                    Column::Extension => c.extension,
                };
                for col in Column::ALL {
                    if ui.selectable_label(on(col), col.label()).clicked() {
                        bars.actions.push(Action::ToggleColumn(col));
                    }
                }
            });
            ui.separator();
            ui.menu_button("Theme", |ui| {
                for (theme, label) in [
                    (Theme::Dark, "Dark"),
                    (Theme::Light, "Light"),
                    (Theme::System, "Match system"),
                ] {
                    if ui
                        .selectable_label(bars.settings.theme == theme, label)
                        .clicked()
                    {
                        bars.actions.push(Action::SetTheme(theme));
                        ui.close();
                    }
                }
            });
        });

        ui.menu_button("Sort", |ui| {
            for key in SortKey::ALL {
                let mark = if tab.sort.key == key {
                    match tab.sort.dir {
                        SortDir::Asc => " (ascending)",
                        SortDir::Desc => " (descending)",
                    }
                } else {
                    ""
                };
                if ui
                    .selectable_label(tab.sort.key == key, format!("{}{mark}", key.label()))
                    .clicked()
                {
                    bars.actions.push(Action::SortBy(key));
                    ui.close();
                }
            }
            ui.separator();
            if ui
                .selectable_label(tab.sort.dirs_first, "Folders first")
                .clicked()
            {
                bars.actions.push(Action::ToggleDirsFirst);
                ui.close();
            }
        });

        ui.menu_button("New", |ui| {
            if ui.button("Folder").clicked() {
                bars.actions.push(Action::NewFolder);
                ui.close();
            }
            if ui.button("File").clicked() {
                bars.actions.push(Action::NewFile);
                ui.close();
            }
            ui.separator();
            if ui.button("Tab").clicked() {
                bars.actions.push(Action::NewTab(None));
                ui.close();
            }
        });

        ui.separator();
        let has_sel = !tab.selection.is_empty();
        if ui.add_enabled(has_sel, egui::Button::new("Copy")).clicked() {
            bars.actions.push(Action::Copy);
        }
        if ui.add_enabled(has_sel, egui::Button::new("Cut")).clicked() {
            bars.actions.push(Action::Cut);
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("Delete"))
            .clicked()
        {
            bars.actions
                .push(Action::DeleteSelected { permanent: false });
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("Rename"))
            .clicked()
        {
            bars.actions.push(Action::BeginRename);
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if matches!(tab.view_mode, ViewMode::Icons | ViewMode::Tiles) {
                let mut size = tab.icon_size;
                if ui
                    .add(
                        egui::Slider::new(&mut size, 32.0..=256.0)
                            .show_value(false)
                            .text("Size"),
                    )
                    .changed()
                {
                    bars.actions.push(Action::SetIconSize(size));
                }
            }
            ui.label(
                egui::RichText::new(format!("{} items", tab.view.len())).color(bars.pal.text_dim),
            );
        });
    });
}

/// Expand a leading `~` so typed paths behave like they do in a shell.
pub fn shellexpand(input: &str) -> String {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), rest);
        }
    }
    trimmed.to_string()
}

/// A square button carrying a painter-drawn arrow.
fn icon_button(ui: &mut Ui, dir: crate::icons::Dir, enabled: bool) -> egui::Response {
    let size = egui::vec2(24.0, 22.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    let color = if !enabled {
        ui.visuals().weak_text_color()
    } else if resp.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    if enabled && resp.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(4),
            ui.visuals().widgets.hovered.weak_bg_fill,
        );
    }
    crate::icons::nav_arrow(ui.painter(), rect, dir, color);
    if enabled {
        resp
    } else {
        // Keep the layout identical but swallow clicks on disabled buttons.
        resp.on_disabled_hover_text("")
    }
}
