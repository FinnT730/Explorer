//! The window chrome: tab strip, command bar and address bar.
//!
//! The arrangement follows Windows 11's File Explorer — tabs on top, then the
//! command bar, then navigation buttons with the address and search chips.

use std::path::{Path, PathBuf};

use egui::{Align, CornerRadius, Layout, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};
use explorer_core::{SortDir, SortKey};

use crate::action::{Action, Column};
use crate::icons::{self, Cmd};
use crate::settings::{Backdrop, Settings, Theme, ViewMode};
use crate::tab::Tab;
use crate::theme::{Palette, RADIUS_CHIP, RADIUS_SMALL};

pub struct Bars<'a> {
    pub settings: &'a Settings,
    pub pal: &'a Palette,
    pub actions: &'a mut Vec<Action>,
    /// `Some` while the address bar is in text-entry mode.
    pub path_edit: &'a mut Option<String>,
    pub search: &'a mut String,
    pub focus_search: bool,
    /// Set once after entering edit mode, to select the whole path.
    pub path_edit_fresh: &'a mut bool,
}

const BAR_H: f32 = 32.0;

// ---------------------------------------------------------------- tab strip

pub fn tab_strip(ui: &mut Ui, tabs: &[Tab], active: usize, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        ui.add_space(4.0);
        for (i, tab) in tabs.iter().enumerate() {
            let selected = i == active;
            let title = tab.title();
            let label = crate::views::list::elide(&title, 130.0);
            let closable = tabs.len() > 1;

            let width =
                36.0 + label.chars().count() as f32 * 6.8 + if closable { 22.0 } else { 0.0 };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, BAR_H), Sense::click());

            if selected {
                // The active tab is the content surface rising into the strip.
                ui.painter().rect_filled(
                    rect,
                    CornerRadius {
                        nw: RADIUS_CHIP,
                        ne: RADIUS_CHIP,
                        sw: 0,
                        se: 0,
                    },
                    bars.pal.surface,
                );
            } else if resp.hovered() {
                ui.painter().rect_filled(
                    rect.shrink2(Vec2::new(0.0, 3.0)),
                    CornerRadius::same(RADIUS_SMALL),
                    bars.pal.hover,
                );
            }

            let icon = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::splat(16.0));
            icons::draw(
                ui.painter(),
                icon,
                explorer_core::FileClass::Folder,
                "",
                bars.pal,
                false,
            );
            ui.painter().text(
                egui::pos2(icon.right() + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &label,
                egui::FontId::proportional(12.5),
                if selected {
                    bars.pal.text
                } else {
                    bars.pal.text_dim
                },
            );

            if closable {
                let close_rect = Rect::from_center_size(
                    egui::pos2(rect.right() - 14.0, rect.center().y),
                    Vec2::splat(18.0),
                );
                let close = ui.interact(close_rect, ui.id().with(("tabclose", i)), Sense::click());
                if close.hovered() {
                    ui.painter().rect_filled(
                        close_rect,
                        CornerRadius::same(RADIUS_SMALL),
                        bars.pal.hover,
                    );
                }
                if selected || resp.hovered() || close.hovered() {
                    icons::cmd(
                        ui.painter(),
                        close_rect.shrink(4.0),
                        Cmd::Close,
                        bars.pal.text_dim,
                    );
                }
                if close.clicked() {
                    bars.actions.push(Action::CloseTab(i));
                }
            }

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
        }

        let (plus_rect, plus) = ui.allocate_exact_size(Vec2::new(28.0, BAR_H), Sense::click());
        let inner = Rect::from_center_size(plus_rect.center(), Vec2::splat(20.0));
        if plus.hovered() {
            ui.painter()
                .rect_filled(inner, CornerRadius::same(RADIUS_SMALL), bars.pal.hover);
        }
        icons::cmd(ui.painter(), inner.shrink(4.0), Cmd::Plus, bars.pal.text);
        if plus.on_hover_text("New tab (Ctrl+T)").clicked() {
            bars.actions.push(Action::NewTab(None));
        }
    });
}

// -------------------------------------------------------------- command bar

pub fn command_bar(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        ui.add_space(4.0);

        let has_sel = !tab.selection.is_empty();
        let can_paste = bars.settings.confirm_delete || true; // paste state is checked by the action

        menu_button(
            ui,
            bars.pal,
            Cmd::New,
            "New",
            |ui, actions| {
                if ui.button("Folder    Ctrl+Shift+N").clicked() {
                    actions.push(Action::NewFolder);
                    ui.close();
                }
                if ui.button("File").clicked() {
                    actions.push(Action::NewFile);
                    ui.close();
                }
                ui.separator();
                if ui.button("Tab    Ctrl+T").clicked() {
                    actions.push(Action::NewTab(None));
                    ui.close();
                }
            },
            bars.actions,
        );

        separator(ui, bars.pal);

        if tool(ui, bars.pal, Cmd::Cut, "Cut (Ctrl+X)", has_sel).clicked() {
            bars.actions.push(Action::Cut);
        }
        if tool(ui, bars.pal, Cmd::Copy, "Copy (Ctrl+C)", has_sel).clicked() {
            bars.actions.push(Action::Copy);
        }
        if tool(ui, bars.pal, Cmd::Paste, "Paste (Ctrl+V)", can_paste).clicked() {
            bars.actions.push(Action::Paste);
        }
        if tool(ui, bars.pal, Cmd::Rename, "Rename (F2)", has_sel).clicked() {
            bars.actions.push(Action::BeginRename);
        }
        if tool(ui, bars.pal, Cmd::Delete, "Delete (Del)", has_sel).clicked() {
            bars.actions
                .push(Action::DeleteSelected { permanent: false });
        }

        separator(ui, bars.pal);

        let sort = tab.sort;
        menu_button(
            ui,
            bars.pal,
            Cmd::Sort,
            "Sort",
            move |ui, actions| {
                for key in SortKey::ALL {
                    let mark = if sort.key == key {
                        match sort.dir {
                            SortDir::Asc => "  ▲",
                            SortDir::Desc => "  ▼",
                        }
                    } else {
                        ""
                    };
                    if ui
                        .selectable_label(sort.key == key, format!("{}{mark}", key.label()))
                        .clicked()
                    {
                        actions.push(Action::SortBy(key));
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(sort.dirs_first, "Folders first")
                    .clicked()
                {
                    actions.push(Action::ToggleDirsFirst);
                    ui.close();
                }
            },
            bars.actions,
        );

        let view_mode = tab.view_mode;
        let settings = bars.settings.clone();
        menu_button(
            ui,
            bars.pal,
            Cmd::Grid,
            "View",
            move |ui, actions| {
                for mode in ViewMode::ALL {
                    if ui
                        .selectable_label(view_mode == mode, mode.label())
                        .clicked()
                    {
                        actions.push(Action::SetViewMode(mode));
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .selectable_label(settings.show_sidebar, "Navigation pane")
                    .clicked()
                {
                    actions.push(Action::ToggleSidebar);
                    ui.close();
                }
                if ui
                    .selectable_label(settings.show_preview, "Preview pane")
                    .clicked()
                {
                    actions.push(Action::TogglePreview);
                    ui.close();
                }
                if ui
                    .selectable_label(settings.show_status_bar, "Status bar")
                    .clicked()
                {
                    actions.push(Action::ToggleStatusBar);
                    ui.close();
                }
                ui.separator();
                if ui
                    .selectable_label(settings.show_hidden, "Hidden items    Ctrl+H")
                    .clicked()
                {
                    actions.push(Action::ToggleHidden);
                    ui.close();
                }
                ui.menu_button("Columns", |ui| {
                    let c = settings.columns;
                    let on = |col: Column| match col {
                        Column::Size => c.size,
                        Column::Kind => c.kind,
                        Column::Modified => c.modified,
                        Column::Created => c.created,
                        Column::Extension => c.extension,
                    };
                    for col in Column::ALL {
                        if ui.selectable_label(on(col), col.label()).clicked() {
                            actions.push(Action::ToggleColumn(col));
                        }
                    }
                });
            },
            bars.actions,
        );

        separator(ui, bars.pal);

        let settings = bars.settings.clone();
        menu_button(
            ui,
            bars.pal,
            Cmd::More,
            "",
            move |ui, actions| {
                if ui.button("Properties    Alt+Enter").clicked() {
                    actions.push(Action::ShowProperties);
                    ui.close();
                }
                if ui.button("Copy path").clicked() {
                    actions.push(Action::CopyPath);
                    ui.close();
                }
                if ui.button("Open in terminal").clicked() {
                    actions.push(Action::OpenTerminal);
                    ui.close();
                }
                if ui.button("Select all    Ctrl+A").clicked() {
                    actions.push(Action::SelectAll);
                    ui.close();
                }
                if ui.button("Invert selection").clicked() {
                    actions.push(Action::InvertSelection);
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Appearance", |ui| {
                    for (theme, label) in [
                        (Theme::Dark, "Dark"),
                        (Theme::Light, "Light"),
                        (Theme::System, "Match system"),
                    ] {
                        if ui
                            .selectable_label(settings.theme == theme, label)
                            .clicked()
                        {
                            actions.push(Action::SetTheme(theme));
                            ui.close();
                        }
                    }
                    if crate::backdrop::supported() {
                        ui.separator();
                        ui.label(
                            egui::RichText::new("Window backdrop")
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                        for backdrop in Backdrop::ALL {
                            if ui
                                .selectable_label(settings.backdrop == backdrop, backdrop.label())
                                .clicked()
                            {
                                actions.push(Action::SetBackdrop(backdrop));
                                ui.close();
                            }
                        }
                    }
                });
                if ui.button("Keyboard shortcuts    F1").clicked() {
                    actions.push(Action::ShowShortcuts);
                    ui.close();
                }
            },
            bars.actions,
        );

        // Right-hand side: pane toggles and the icon-size slider.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(4.0);
            if toggle(
                ui,
                bars.pal,
                Cmd::PreviewPane,
                "Preview pane (Ctrl+P)",
                bars.settings.show_preview,
            )
            .clicked()
            {
                bars.actions.push(Action::TogglePreview);
            }
            if toggle(
                ui,
                bars.pal,
                Cmd::NavPane,
                "Navigation pane (Ctrl+B)",
                bars.settings.show_sidebar,
            )
            .clicked()
            {
                bars.actions.push(Action::ToggleSidebar);
            }
            if matches!(tab.view_mode, ViewMode::Icons | ViewMode::Tiles) {
                let mut size = tab.icon_size;
                if ui
                    .add(
                        egui::Slider::new(&mut size, 32.0..=256.0)
                            .show_value(false)
                            .handle_shape(egui::style::HandleShape::Circle),
                    )
                    .on_hover_text("Icon size")
                    .changed()
                {
                    bars.actions.push(Action::SetIconSize(size));
                }
            }
        });
    });
}

// -------------------------------------------------------------- address bar

pub fn address_bar(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
        ui.add_space(4.0);

        if nav_button(ui, bars.pal, icons::Dir::Left, tab.history.can_go_back())
            .on_hover_text("Back (Alt+Left)")
            .clicked()
        {
            bars.actions.push(Action::Back);
        }
        if nav_button(
            ui,
            bars.pal,
            icons::Dir::Right,
            tab.history.can_go_forward(),
        )
        .on_hover_text("Forward (Alt+Right)")
        .clicked()
        {
            bars.actions.push(Action::Forward);
        }
        if nav_button(ui, bars.pal, icons::Dir::Up, tab.history.can_go_up())
            .on_hover_text("Up (Alt+Up)")
            .clicked()
        {
            bars.actions.push(Action::Up);
        }
        ui.add_space(4.0);

        // The search chip keeps a fixed width on the right; the address chip
        // takes everything that is left.
        let search_w = (ui.available_width() * 0.28).clamp(140.0, 280.0);
        let address_w = (ui.available_width() - search_w - 10.0).max(120.0);
        address_chip(ui, tab, bars, address_w);
        ui.add_space(4.0);
        search_chip(ui, tab, bars, ui.available_width().max(100.0) - 4.0);
    });
}

fn address_chip(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>, width: f32) {
    let (chip, chip_resp) = ui.allocate_exact_size(Vec2::new(width, BAR_H), Sense::click());
    ui.painter()
        .rect_filled(chip, CornerRadius::same(RADIUS_CHIP), bars.pal.field);
    ui.painter().rect_stroke(
        chip,
        CornerRadius::same(RADIUS_CHIP),
        Stroke::new(1.0, bars.pal.field_stroke),
        StrokeKind::Inside,
    );

    let path = tab.path().to_path_buf();

    // --- text entry mode
    if bars.path_edit.is_some() {
        let inner = chip.shrink2(Vec2::new(8.0, 4.0));
        let field_id = ui.id().with("address-edit");
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        let buf = bars.path_edit.as_mut().expect("checked above");
        let resp = child.add(
            egui::TextEdit::singleline(buf)
                .id(field_id)
                .frame(egui::Frame::NONE)
                .desired_width(inner.width())
                .hint_text("Type a path and press Enter"),
        );
        if !resp.has_focus() {
            resp.request_focus();
        }
        if *bars.path_edit_fresh {
            // Entering the bar selects the whole path, as it does in Explorer.
            crate::dialogs::select_range(ui.ctx(), field_id, 0..buf.chars().count());
            *bars.path_edit_fresh = false;
        }
        let (enter, escape) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if enter {
            let typed = expand_path(buf);
            *bars.path_edit = None;
            bars.actions.push(Action::Navigate(typed));
        } else if escape || resp.clicked_elsewhere() {
            *bars.path_edit = None;
        }
        return;
    }

    // --- breadcrumb mode
    let refresh_rect = Rect::from_center_size(
        egui::pos2(chip.right() - 17.0, chip.center().y),
        Vec2::splat(24.0),
    );
    let crumbs_rect = Rect::from_min_max(
        chip.min + Vec2::new(6.0, 3.0),
        egui::pos2(refresh_rect.left() - 2.0, chip.max.y - 3.0),
    );

    let font = egui::FontId::proportional(12.5);
    let components: Vec<_> = path.components().collect();

    // Measure every segment, then keep as many trailing ones as fit: the
    // folder you are in matters more than the root, so the head is elided.
    let widths: Vec<f32> = components
        .iter()
        .map(|comp| {
            let name = crumb_label(comp);
            ui.painter()
                .layout_no_wrap(name, font.clone(), bars.pal.text)
                .size()
                .x
                + 12.0
        })
        .collect();
    const CHEVRON_W: f32 = 16.0;
    const ELLIPSIS_W: f32 = 18.0;

    let avail = crumbs_rect.width();
    let mut first = 0usize;
    loop {
        let mut total: f32 = widths[first..].iter().sum();
        total += CHEVRON_W * (components.len() - first).saturating_sub(1) as f32;
        if first > 0 {
            total += ELLIPSIS_W + CHEVRON_W;
        }
        if total <= avail || first + 1 >= components.len() {
            break;
        }
        first += 1;
    }

    let mut x = crumbs_rect.left();
    if first > 0 {
        let ell = Rect::from_min_size(
            egui::pos2(x, crumbs_rect.top()),
            Vec2::new(ELLIPSIS_W, crumbs_rect.height()),
        );
        let resp = ui.interact(ell, ui.id().with("crumb-ellipsis"), Sense::click());
        if resp.hovered() {
            ui.painter()
                .rect_filled(ell, CornerRadius::same(RADIUS_SMALL), bars.pal.hover);
        }
        ui.painter().text(
            ell.center(),
            egui::Align2::CENTER_CENTER,
            "…",
            font.clone(),
            bars.pal.text_dim,
        );
        // The hidden head is still reachable: list it from the ellipsis.
        let mut hidden: Vec<(String, PathBuf)> = Vec::new();
        let mut acc = PathBuf::new();
        for comp in components.iter().take(first) {
            acc.push(comp.as_os_str());
            hidden.push((crumb_label(comp), acc.clone()));
        }
        egui::Popup::menu(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                for (label, target) in hidden {
                    if ui.button(label).clicked() {
                        bars.actions.push(Action::Navigate(target));
                    }
                }
            });
        x += ELLIPSIS_W;
        icons::chevron(
            ui.painter(),
            egui::pos2(x + CHEVRON_W * 0.5, crumbs_rect.center().y),
            8.0,
            false,
            bars.pal.text_dim,
        );
        x += CHEVRON_W;
    }

    let mut acc = PathBuf::new();
    for (i, comp) in components.iter().enumerate() {
        acc.push(comp.as_os_str());
        if i < first {
            continue;
        }
        let name = crumb_label(comp);
        let seg_rect = Rect::from_min_size(
            egui::pos2(x, crumbs_rect.top()),
            Vec2::new(widths[i], crumbs_rect.height()),
        );
        let seg = ui.interact(seg_rect, ui.id().with(("crumb", i)), Sense::click());
        if seg.hovered() {
            ui.painter()
                .rect_filled(seg_rect, CornerRadius::same(RADIUS_SMALL), bars.pal.hover);
        }
        ui.painter().text(
            seg_rect.center(),
            egui::Align2::CENTER_CENTER,
            &name,
            font.clone(),
            bars.pal.text,
        );
        if seg.clicked() {
            bars.actions.push(Action::Navigate(acc.clone()));
        }
        if seg.middle_clicked() {
            bars.actions.push(Action::OpenInNewTab(acc.clone()));
        }
        address_menu(&seg, &path, bars);
        x += widths[i];

        if i + 1 < components.len() {
            let chev_rect = Rect::from_min_size(
                egui::pos2(x, crumbs_rect.top()),
                Vec2::new(CHEVRON_W, crumbs_rect.height()),
            );
            let chev = ui.interact(chev_rect, ui.id().with(("crumb-chev", i)), Sense::click());
            if chev.hovered() {
                ui.painter().rect_filled(
                    chev_rect,
                    CornerRadius::same(RADIUS_SMALL),
                    bars.pal.hover,
                );
            }
            icons::chevron(
                ui.painter(),
                chev_rect.center(),
                8.0,
                false,
                bars.pal.text_dim,
            );
            // Clicking a separator lists that folder's children, like Explorer.
            subfolder_popup(&chev, &acc, bars);
            x += CHEVRON_W;
        }
    }

    // Refresh, inside the chip at the right edge, as in Explorer.
    let refresh = ui.interact(
        refresh_rect,
        ui.id().with("address-refresh"),
        Sense::click(),
    );
    if refresh.hovered() {
        ui.painter().rect_filled(
            refresh_rect.shrink(2.0),
            CornerRadius::same(RADIUS_SMALL),
            bars.pal.hover,
        );
    }
    icons::nav_arrow(
        ui.painter(),
        refresh_rect,
        icons::Dir::Refresh,
        bars.pal.text,
    );
    if refresh.on_hover_text("Refresh (F5)").clicked() {
        bars.actions.push(Action::Refresh);
    }

    // Blank space in the chip behaves like Explorer's: click to edit the path.
    let blank = Rect::from_min_max(egui::pos2(x, crumbs_rect.top()), crumbs_rect.max);
    let blank_resp = ui.interact(blank, ui.id().with("address-blank"), Sense::click());
    if blank_resp.clicked() {
        *bars.path_edit = Some(path.display().to_string());
        *bars.path_edit_fresh = true;
    }
    if blank_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    address_menu(&blank_resp, &path, bars);
    let _ = chip_resp;
}

/// Right-click menu shared by every part of the address bar.
fn address_menu(resp: &Response, path: &Path, bars: &mut Bars<'_>) {
    let path = path.to_path_buf();
    resp.context_menu(|ui| {
        ui.set_min_width(180.0);
        if ui.button("Copy address").clicked() {
            ui.ctx().copy_text(path.display().to_string());
            bars.actions.push(Action::Toast("Address copied".into()));
            ui.close();
        }
        if ui.button("Edit address").clicked() {
            *bars.path_edit = Some(path.display().to_string());
            *bars.path_edit_fresh = true;
            ui.close();
        }
        ui.separator();
        if ui.button("Open in new tab").clicked() {
            bars.actions.push(Action::OpenInNewTab(path.clone()));
            ui.close();
        }
        if ui.button("Open in terminal").clicked() {
            bars.actions.push(Action::OpenTerminal);
            ui.close();
        }
    });
}

fn crumb_label(comp: &std::path::Component<'_>) -> String {
    match comp {
        std::path::Component::RootDir => "/".to_string(),
        other => other.as_os_str().to_string_lossy().into_owned(),
    }
}

/// Drop-down of a breadcrumb's subfolders, opened from its chevron.
fn subfolder_popup(chevron: &Response, dir: &Path, bars: &mut Bars<'_>) {
    egui::Popup::menu(chevron)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(190.0);
            ui.set_max_height(420.0);
            let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
                .map(|rd| {
                    rd.filter_map(Result::ok)
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.path())
                        .collect()
                })
                .unwrap_or_default();
            dirs.sort();
            if dirs.is_empty() {
                ui.label(egui::RichText::new("No subfolders").color(bars.pal.text_dim));
                return;
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for d in dirs.into_iter().take(300) {
                    if ui.button(explorer_core::entry::file_name_of(&d)).clicked() {
                        bars.actions.push(Action::Navigate(d));
                    }
                }
            });
        });
}

fn search_chip(ui: &mut Ui, tab: &Tab, bars: &mut Bars<'_>, width: f32) {
    let (chip, _) = ui.allocate_exact_size(Vec2::new(width, BAR_H), Sense::hover());
    ui.painter()
        .rect_filled(chip, CornerRadius::same(RADIUS_CHIP), bars.pal.field);
    ui.painter().rect_stroke(
        chip,
        CornerRadius::same(RADIUS_CHIP),
        Stroke::new(1.0, bars.pal.field_stroke),
        StrokeKind::Inside,
    );

    let icon = Rect::from_min_size(chip.min + Vec2::new(7.0, 8.0), Vec2::splat(16.0));
    icons::cmd(ui.painter(), icon, Cmd::Search, bars.pal.text_dim);

    let clear_w = if bars.search.is_empty() { 0.0 } else { 22.0 };
    let field = Rect::from_min_max(
        egui::pos2(icon.right() + 6.0, chip.top() + 4.0),
        egui::pos2(chip.right() - 8.0 - clear_w, chip.bottom() - 4.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
    let resp = child.add(
        egui::TextEdit::singleline(bars.search)
            .frame(egui::Frame::NONE)
            .desired_width(field.width())
            .hint_text(if tab.filter.is_searching() {
                "Searching…"
            } else {
                "Search"
            }),
    );
    if bars.focus_search {
        resp.request_focus();
    }
    if resp.changed() {
        bars.actions.push(Action::SetQuery(bars.search.clone()));
    }
    if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        bars.actions.push(Action::SearchHere);
    }

    if clear_w > 0.0 {
        let clear_rect = Rect::from_center_size(
            egui::pos2(chip.right() - 15.0, chip.center().y),
            Vec2::splat(20.0),
        );
        let clear = ui.interact(clear_rect, ui.id().with("search-clear"), Sense::click());
        if clear.hovered() {
            ui.painter()
                .rect_filled(clear_rect, CornerRadius::same(RADIUS_SMALL), bars.pal.hover);
        }
        icons::cmd(
            ui.painter(),
            clear_rect.shrink(5.0),
            Cmd::Close,
            bars.pal.text_dim,
        );
        if clear.on_hover_text("Clear search").clicked() {
            bars.actions.push(Action::ClearSearch);
        }
    }
}

// ------------------------------------------------------------------ widgets

fn separator(ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(9.0, BAR_H), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top() + 7.0),
            egui::pos2(rect.center().x, rect.bottom() - 7.0),
        ],
        Stroke::new(1.0, pal.separator),
    );
}

/// Square icon button for the command bar.
fn tool(ui: &mut Ui, pal: &Palette, kind: Cmd, tip: &str, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, BAR_H), Sense::click());
    let color = if enabled {
        pal.text
    } else {
        pal.text_dim.gamma_multiply(0.6)
    };
    if enabled && (resp.hovered() || resp.is_pointer_button_down_on()) {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(RADIUS_SMALL),
            pal.hover,
        );
    }
    icons::cmd(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        kind,
        color,
    );
    let resp = resp.on_hover_text(tip);
    if enabled {
        resp
    } else {
        // Keep the slot but swallow the click.
        let mut dead = resp;
        dead.flags.remove(egui::response::Flags::CLICKED);
        dead
    }
}

/// Icon button that reads as pressed while its pane is open.
fn toggle(ui: &mut Ui, pal: &Palette, kind: Cmd, tip: &str, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(34.0, BAR_H), Sense::click());
    let inner = rect.shrink2(Vec2::new(1.0, 2.0));
    if on {
        ui.painter()
            .rect_filled(inner, CornerRadius::same(RADIUS_SMALL), pal.selection);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(inner, CornerRadius::same(RADIUS_SMALL), pal.hover);
    }
    icons::cmd(
        ui.painter(),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        kind,
        pal.text,
    );
    resp.on_hover_text(tip)
}

/// Icon + label + chevron, opening a drop-down menu.
fn menu_button(
    ui: &mut Ui,
    pal: &Palette,
    kind: Cmd,
    label: &str,
    contents: impl FnOnce(&mut Ui, &mut Vec<Action>),
    actions: &mut Vec<Action>,
) {
    let text_w = if label.is_empty() {
        0.0
    } else {
        ui.painter()
            .layout_no_wrap(label.to_owned(), egui::FontId::proportional(12.5), pal.text)
            .size()
            .x
            + 6.0
    };
    let width = 26.0 + text_w + if label.is_empty() { 0.0 } else { 14.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, BAR_H), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    if resp.hovered() || open {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(RADIUS_SMALL),
            pal.hover,
        );
    }
    let icon = Rect::from_min_size(
        egui::pos2(rect.left() + 6.0, rect.center().y - 8.0),
        Vec2::splat(16.0),
    );
    icons::cmd(ui.painter(), icon, kind, pal.text);
    if !label.is_empty() {
        ui.painter().text(
            egui::pos2(icon.right() + 5.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.5),
            pal.text,
        );
        icons::chevron(
            ui.painter(),
            egui::pos2(rect.right() - 9.0, rect.center().y),
            8.0,
            true,
            pal.text_dim,
        );
    }

    egui::Popup::menu(&resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(200.0);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            contents(ui, actions);
        });
}

fn nav_button(ui: &mut Ui, pal: &Palette, dir: icons::Dir, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(32.0, BAR_H), Sense::click());
    let color = if enabled {
        pal.text
    } else {
        pal.text_dim.gamma_multiply(0.5)
    };
    if enabled && resp.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(1.0, 2.0)),
            CornerRadius::same(RADIUS_SMALL),
            pal.hover,
        );
    }
    icons::nav_arrow(ui.painter(), rect, dir, color);
    if enabled {
        resp
    } else {
        let mut dead = resp;
        dead.flags.remove(egui::response::Flags::CLICKED);
        dead
    }
}

/// Expand a leading `~` so typed paths behave like they do in a shell.
pub fn expand_path(input: &str) -> PathBuf {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return PathBuf::from(format!("{}{}", home.display(), rest));
        }
    }
    PathBuf::from(trimmed)
}

/// Thin line under the chrome, separating it from the content surface.
pub fn chrome_edge(ui: &Ui, rect: Rect, pal: &Palette) {
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, pal.separator),
    );
}

/// Fill used behind the listing, so the content area reads as one surface.
pub fn surface_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(pal.surface)
        .inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        })
}
