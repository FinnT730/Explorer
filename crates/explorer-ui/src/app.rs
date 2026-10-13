//! The application: panels, state, keyboard handling and the action loop.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Context, Key};
use explorer_core::{
    ops::{ClipMode, Clipboard, Ops, Progress, Task},
    places, DirWatcher, Places, ScanResult, Scanner,
};
use explorer_thumbs::Thumbnails;

use crate::action::{Action, Column};
use crate::dialogs::{self, Dialog, DialogOutcome, Measured};
use crate::settings::{Settings, Theme, ViewMode};
use crate::sidebar::{self, SidebarState};
use crate::statusbar;
use crate::tab::Tab;
use crate::textures::TextureStore;
use crate::theme::{self, Palette};
use crate::toolbar::{self, Bars};
use crate::views::{self, ViewCtx};

/// A transient message shown in the bottom-right corner.
struct Toast {
    text: String,
    born: Instant,
    error: bool,
}

/// An in-flight file operation, for the progress strip.
struct Running {
    id: u64,
    label: &'static str,
    done: u64,
    total: u64,
    current: String,
}

pub struct App {
    settings: Settings,
    tabs: Vec<Tab>,
    active: usize,
    places: Places,
    scanner: Scanner,
    ops: Ops,
    watcher: DirWatcher,
    thumbs: Thumbnails,
    textures: TextureStore,
    clipboard: Clipboard,
    sidebar: SidebarState,
    dialog: Option<Dialog>,
    actions: Vec<Action>,
    toasts: Vec<Toast>,
    running: Option<Running>,
    path_edit: Option<String>,
    path_edit_fresh: bool,
    search: String,
    focus_search: bool,
    theme_applied: Option<Theme>,
    volumes_checked: Instant,
    /// True when the window really is translucent, so the theme can tint its
    /// surfaces instead of painting them solid.
    translucent: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        let settings = Settings::load();
        let dark = theme::is_dark(settings.theme, &cc.egui_ctx);
        // The backdrop has to go on before the first frame; if the platform
        // declines it, the theme stays opaque so nothing looks washed out.
        let translucent = crate::backdrop::apply(cc, settings.backdrop, dark);
        theme::install_system_font(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, settings.theme, translucent);

        // Every worker wakes the UI through the same repaint callback, so
        // results show up without polling or spinning the event loop.
        let ctx = cc.egui_ctx.clone();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(move || ctx.request_repaint());

        let workers = std::thread::available_parallelism()
            .map(|n| (n.get().saturating_sub(1)).clamp(1, 6))
            .unwrap_or(2);
        let thumbs = Thumbnails::new(workers, settings.thumbnail_cache, wake.clone());

        let mut starts: Vec<PathBuf> = Vec::new();
        if let Some(p) = start {
            starts.push(p);
        } else {
            starts.extend(settings.last_tabs.iter().filter(|p| p.is_dir()).cloned());
        }
        if starts.is_empty() {
            starts.push(places::default_start_dir());
        }

        let tabs: Vec<Tab> = starts.into_iter().map(|p| Tab::new(p, &settings)).collect();

        let mut app = Self {
            places: Places::load(settings.pinned.clone()),
            scanner: Scanner::new(wake.clone()),
            ops: Ops::new(wake.clone()),
            watcher: DirWatcher::new(wake),
            thumbs,
            textures: TextureStore::default(),
            clipboard: Clipboard::default(),
            sidebar: SidebarState::default(),
            dialog: None,
            actions: Vec::new(),
            toasts: Vec::new(),
            running: None,
            path_edit: None,
            path_edit_fresh: false,
            search: String::new(),
            focus_search: false,
            theme_applied: Some(settings.theme),
            volumes_checked: Instant::now(),
            translucent,
            active: 0,
            tabs,
            settings,
        };
        for i in 0..app.tabs.len() {
            app.start_scan(i);
        }
        app
    }

    fn palette(&self, ctx: &Context) -> Palette {
        theme::palette(theme::is_dark(self.settings.theme, ctx), self.translucent)
    }

    fn tab(&self) -> &Tab {
        &self.tabs[self.active]
    }

    fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toasts.push(Toast {
            text: text.into(),
            born: Instant::now(),
            error,
        });
    }

    // ---------------------------------------------------------------- scanning

    fn start_scan(&mut self, index: usize) {
        let path = self.tabs[index].path().to_path_buf();
        self.thumbs.new_generation();
        let id = self.scanner.list(&path);
        let tab = &mut self.tabs[index];
        tab.pending_scan = Some(id);
        tab.pending_search = None;
        tab.error = None;
        if index == self.active {
            self.watcher.watch(&path);
        }
    }

    fn start_search(&mut self, index: usize) {
        let path = self.tabs[index].path().to_path_buf();
        let query = self.tabs[index].filter.query.clone();
        if query.trim().is_empty() {
            self.start_scan(index);
            return;
        }
        let id = self.scanner.search(&path, query);
        let tab = &mut self.tabs[index];
        tab.entries.clear();
        tab.view.clear();
        tab.pending_search = Some(id);
        tab.pending_scan = None;
    }

    fn collect_results(&mut self) {
        for result in self.scanner.drain() {
            // Properties measurement is routed to the open dialog.
            if let ScanResult::Measured {
                id,
                bytes,
                files,
                dirs,
                ..
            } = &result
            {
                if let Some(Dialog::Properties {
                    measured, request, ..
                }) = self.dialog.as_mut()
                {
                    if *request == Some(*id) {
                        *measured = Some(Measured {
                            bytes: *bytes,
                            files: *files,
                            dirs: *dirs,
                        });
                    }
                }
                continue;
            }

            let id = result.id();
            let Some(index) = self
                .tabs
                .iter()
                .position(|t| t.pending_scan == Some(id) || t.pending_search == Some(id))
            else {
                continue; // Result for a navigation we already left.
            };
            let tab = &mut self.tabs[index];
            match result {
                ScanResult::Listing { entries, .. } => {
                    tab.entries = entries;
                    tab.pending_scan = None;
                    tab.needs_rebuild = true;
                }
                ScanResult::SearchBatch { entries, done, .. } => {
                    tab.entries.extend(entries);
                    tab.needs_rebuild = true;
                    if done {
                        tab.pending_search = None;
                    }
                }
                ScanResult::Failed { error, .. } => {
                    tab.pending_scan = None;
                    tab.pending_search = None;
                    tab.error = Some(error);
                }
                ScanResult::Measured { .. } => unreachable!(),
            }
        }

        for progress in self.ops.drain() {
            match progress {
                Progress::Started { id, label, total } => {
                    self.running = Some(Running {
                        id,
                        label,
                        done: 0,
                        total,
                        current: String::new(),
                    });
                }
                Progress::Advanced {
                    id,
                    done,
                    total,
                    current,
                } => {
                    if let Some(r) = self.running.as_mut() {
                        if r.id == id {
                            r.done = done;
                            r.total = total;
                            r.current = explorer_core::entry::file_name_of(&current);
                        }
                    }
                }
                Progress::Finished { id, task, errors } => {
                    if self.running.as_ref().map(|r| r.id) == Some(id) {
                        self.running = None;
                    }
                    if errors.is_empty() {
                        self.toast(format!("{} finished", task.label()), false);
                    } else {
                        let body = errors.join("\n");
                        self.toast(format!("{} failed", task.label()), true);
                        self.dialog = Some(Dialog::Error {
                            title: format!("{} did not complete", task.label()),
                            body,
                        });
                    }
                    // Refresh any tab showing a directory the task touched.
                    let dirs = task.touched_dirs();
                    let affected: Vec<usize> = self
                        .tabs
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| dirs.iter().any(|d| d == t.path()))
                        .map(|(i, _)| i)
                        .collect();
                    for i in affected {
                        self.start_scan(i);
                    }
                }
            }
        }
    }

    // ----------------------------------------------------------------- actions

    fn apply(&mut self, action: Action, ctx: &Context) {
        match action {
            Action::Navigate(path) => {
                let path = resolve_dir(&path);
                if self.tab_mut().history.push(path) {
                    self.tab_mut().clear_selection();
                    self.search.clear();
                    self.tab_mut().filter.query.clear();
                    self.start_scan(self.active);
                }
            }
            Action::OpenInNewTab(path) => {
                let tab = Tab::new(resolve_dir(&path), &self.settings);
                self.tabs.push(tab);
                self.active = self.tabs.len() - 1;
                self.start_scan(self.active);
            }
            Action::Activate(path) => {
                if path.is_dir() {
                    self.apply(Action::Navigate(path), ctx);
                } else if let Err(e) = explorer_core::ops::open_default(&path) {
                    self.toast(format!("Could not open: {e}"), true);
                }
            }
            Action::Back => {
                if self.tab_mut().history.back().is_some() {
                    self.tab_mut().clear_selection();
                    self.start_scan(self.active);
                }
            }
            Action::Forward => {
                if self.tab_mut().history.forward().is_some() {
                    self.tab_mut().clear_selection();
                    self.start_scan(self.active);
                }
            }
            Action::Up => {
                if self.tab_mut().history.up().is_some() {
                    self.tab_mut().clear_selection();
                    self.start_scan(self.active);
                }
            }
            Action::Refresh => {
                self.places = Places::load(self.settings.pinned.clone());
                if self.tab().filter.is_searching() {
                    self.start_search(self.active);
                } else {
                    self.start_scan(self.active);
                }
            }

            Action::NewTab(path) => {
                let path = path.unwrap_or_else(|| self.tab().path().to_path_buf());
                self.tabs.push(Tab::new(path, &self.settings));
                self.active = self.tabs.len() - 1;
                self.start_scan(self.active);
            }
            Action::DuplicateTab(i) => {
                if let Some(t) = self.tabs.get(i) {
                    let path = t.path().to_path_buf();
                    self.apply(Action::OpenInNewTab(path), ctx);
                }
            }
            Action::CloseTab(i) => {
                if self.tabs.len() > 1 && i < self.tabs.len() {
                    self.tabs.remove(i);
                    self.active = self.active.min(self.tabs.len() - 1);
                    let path = self.tab().path().to_path_buf();
                    self.watcher.watch(&path);
                }
            }
            Action::SelectTab(i) => {
                if i < self.tabs.len() {
                    self.active = i;
                    self.search = self.tab().filter.query.clone();
                    let path = self.tab().path().to_path_buf();
                    self.watcher.watch(&path);
                    if self.tab().entries.is_empty() && !self.tab().is_loading() {
                        self.start_scan(i);
                    }
                }
            }

            Action::SetViewMode(mode) => {
                self.tab_mut().view_mode = mode;
                self.settings.view_mode = mode;
            }
            Action::SetIconSize(size) => {
                self.tab_mut().icon_size = size;
                self.settings.icon_size = size;
            }
            Action::SortBy(key) => {
                let tab = self.tab_mut();
                tab.sort.toggle(key);
                tab.needs_rebuild = true;
                self.settings.sort = self.tab().sort;
            }
            Action::ToggleDirsFirst => {
                let tab = self.tab_mut();
                tab.sort.dirs_first = !tab.sort.dirs_first;
                tab.needs_rebuild = true;
                self.settings.sort = self.tab().sort;
            }
            Action::ToggleHidden => {
                self.settings.show_hidden = !self.settings.show_hidden;
                for tab in &mut self.tabs {
                    tab.filter.show_hidden = self.settings.show_hidden;
                    tab.needs_rebuild = true;
                }
            }
            Action::ToggleSidebar => self.settings.show_sidebar = !self.settings.show_sidebar,
            Action::TogglePreview => self.settings.show_preview = !self.settings.show_preview,
            Action::ToggleStatusBar => {
                self.settings.show_status_bar = !self.settings.show_status_bar
            }
            Action::ToggleColumn(col) => {
                let c = &mut self.settings.columns;
                match col {
                    Column::Size => c.size = !c.size,
                    Column::Kind => c.kind = !c.kind,
                    Column::Modified => c.modified = !c.modified,
                    Column::Created => c.created = !c.created,
                    Column::Extension => c.extension = !c.extension,
                }
            }
            Action::SetTheme(t) => {
                self.settings.theme = t;
                theme::apply(ctx, t, self.translucent);
                self.theme_applied = Some(t);
            }

            Action::SelectAll => self.tab_mut().select_all(),
            Action::InvertSelection => self.tab_mut().invert_selection(),
            Action::ClearSelection => self.tab_mut().clear_selection(),
            Action::SelectIndex {
                index,
                extend,
                toggle,
            } => {
                let tab = self.tab_mut();
                if extend {
                    tab.select_range(index);
                } else if toggle {
                    tab.toggle(index);
                } else {
                    tab.select_only(index);
                }
            }

            Action::SetQuery(q) => {
                let tab = self.tab_mut();
                tab.filter.query = q;
                tab.needs_rebuild = true;
                // Typing filters the current listing; Enter searches subfolders.
                if tab.pending_search.is_some() {
                    tab.pending_search = None;
                }
            }
            Action::SearchHere => {
                if self.tab().filter.is_searching() {
                    self.start_search(self.active);
                }
            }
            Action::ClearSearch => {
                self.search.clear();
                let tab = self.tab_mut();
                tab.filter.query.clear();
                tab.needs_rebuild = true;
                self.start_scan(self.active);
            }

            Action::Copy => {
                let paths = self.tab().selected_paths();
                if !paths.is_empty() {
                    let n = paths.len();
                    self.clipboard.set(paths, ClipMode::Copy);
                    self.toast(format!("Copied {n} item{}", plural(n)), false);
                }
            }
            Action::Cut => {
                let paths = self.tab().selected_paths();
                if !paths.is_empty() {
                    let n = paths.len();
                    self.clipboard.set(paths, ClipMode::Cut);
                    self.toast(format!("Cut {n} item{}", plural(n)), false);
                }
            }
            Action::Paste => {
                if self.clipboard.is_empty() {
                    return;
                }
                let dst = self.tab().path().to_path_buf();
                let srcs = self.clipboard.paths.clone();
                let task = match self.clipboard.mode {
                    Some(ClipMode::Cut) => Task::Move { srcs, dst_dir: dst },
                    _ => Task::Copy { srcs, dst_dir: dst },
                };
                self.ops.submit(task);
                if self.clipboard.mode == Some(ClipMode::Cut) {
                    self.clipboard.clear();
                }
            }
            Action::DropOnto { paths, dir, copy } => {
                let task = if copy {
                    Task::Copy {
                        srcs: paths,
                        dst_dir: dir,
                    }
                } else {
                    Task::Move {
                        srcs: paths,
                        dst_dir: dir,
                    }
                };
                self.ops.submit(task);
            }
            Action::DeleteSelected { permanent } => {
                let paths = self.tab().selected_paths();
                if paths.is_empty() {
                    return;
                }
                if self.settings.confirm_delete || permanent {
                    self.dialog = Some(Dialog::ConfirmDelete { paths, permanent });
                } else {
                    self.ops.submit(Task::Trash(paths));
                }
            }
            Action::BeginRename => {
                let target = self
                    .tab()
                    .selected_entries()
                    .next()
                    .map(|e| (e.path.clone(), e.name.clone()));
                if let Some((path, name)) = target {
                    // Details renames in place; the other views have no room
                    // for an editor, so they get a small dialog instead.
                    if self.tab().view_mode == ViewMode::Details {
                        self.tab_mut().renaming = Some((path, name));
                    } else {
                        self.dialog = Some(Dialog::Rename { path, name });
                    }
                }
            }
            Action::CommitRename { from, to_name } => {
                self.tab_mut().renaming = None;
                self.apply_rename(from, to_name);
            }
            Action::CancelRename => self.tab_mut().renaming = None,
            Action::NewFolder => {
                self.dialog = Some(Dialog::NewFolder {
                    dir: self.tab().path().to_path_buf(),
                    name: String::from("New folder"),
                });
            }
            Action::NewFile => {
                self.dialog = Some(Dialog::NewFile {
                    dir: self.tab().path().to_path_buf(),
                    name: String::from("New file.txt"),
                });
            }
            Action::CopyPath => {
                let paths = self.tab().selected_paths();
                let text = if paths.is_empty() {
                    self.tab().path().display().to_string()
                } else {
                    paths
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                ctx.copy_text(text);
                self.toast("Path copied", false);
            }
            Action::OpenTerminal => {
                let dir = self.tab().path().to_path_buf();
                if let Err(e) = explorer_core::ops::open_terminal(&dir) {
                    self.toast(format!("No terminal: {e}"), true);
                }
            }
            Action::RevealInOsFileManager => {
                let path = self
                    .tab()
                    .selected_paths()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| self.tab().path().to_path_buf());
                if let Err(e) = explorer_core::ops::reveal(&path) {
                    self.toast(format!("Could not reveal: {e}"), true);
                }
            }
            Action::ShowProperties => {
                let mut paths = self.tab().selected_paths();
                if paths.is_empty() {
                    paths.push(self.tab().path().to_path_buf());
                }
                // Folders need a recursive measurement, which runs off-thread.
                let request = paths
                    .iter()
                    .any(|p| p.is_dir())
                    .then(|| self.scanner.measure(paths[0].clone()));
                self.dialog = Some(Dialog::Properties {
                    paths,
                    measured: None,
                    request,
                });
            }

            Action::Pin(path) => {
                if !self.settings.pinned.contains(&path) {
                    self.settings.pinned.push(path);
                    self.places = Places::load(self.settings.pinned.clone());
                }
            }
            Action::Unpin(path) => {
                self.settings.pinned.retain(|p| *p != path);
                self.places = Places::load(self.settings.pinned.clone());
            }
            Action::RefreshVolumes => {
                self.places = Places::load(self.settings.pinned.clone());
            }

            Action::SetBackdrop(b) => {
                self.settings.backdrop = b;
                self.toast("Backdrop applies the next time Explorer starts", false);
            }
            Action::ShowShortcuts => self.dialog = Some(Dialog::Shortcuts),
            Action::Toast(text) => self.toast(text, false),
            Action::CloseDialog => self.dialog = None,
            Action::Confirm => self.confirm_dialog(),
            Action::Dismiss(i) => {
                if i < self.toasts.len() {
                    self.toasts.remove(i);
                }
            }
            Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
        }
    }

    fn apply_rename(&mut self, from: PathBuf, to_name: String) {
        let trimmed = to_name.trim().to_string();
        if trimmed.is_empty() || !dialogs::is_valid_name(&trimmed) {
            self.toast("That name is not allowed", true);
            return;
        }
        let to = from.with_file_name(&trimmed);
        if to != from {
            self.ops.submit(Task::Rename { from, to });
        }
    }

    fn confirm_dialog(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            Dialog::NewFolder { dir, name } => {
                self.ops.submit(Task::NewFolder(dir.join(name.trim())));
            }
            Dialog::NewFile { dir, name } => {
                self.ops.submit(Task::NewFile(dir.join(name.trim())));
            }
            Dialog::Rename { path, name } => {
                self.apply_rename(path, name);
            }
            Dialog::ConfirmDelete { paths, permanent } => {
                self.ops.submit(if permanent {
                    Task::Delete(paths)
                } else {
                    Task::Trash(paths)
                });
            }
            other => self.dialog = Some(other),
        }
    }

    // ---------------------------------------------------------------- keyboard

    fn keyboard(&mut self, ctx: &Context) {
        // While a text field has the keyboard, only window-level shortcuts
        // fire: the field keeps the plain keys and the editing combos.
        let typing = ctx.egui_wants_keyboard_input();
        // egui turns the clipboard chords into their own events, so they
        // never show up as key presses.
        let (mods, typed, clip) = ctx.input(|i| {
            let mut typed = String::new();
            let mut clip = (false, false, false);
            for event in &i.events {
                match event {
                    egui::Event::Text(t) => typed.push_str(t),
                    egui::Event::Copy => clip.0 = true,
                    egui::Event::Cut => clip.1 = true,
                    egui::Event::Paste(_) => clip.2 = true,
                    _ => {}
                }
            }
            (i.modifiers, typed, clip)
        });
        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key));
        let ctrl = mods.command || mods.ctrl;

        if pressed(Key::F1) {
            self.dialog = Some(Dialog::Shortcuts);
        }
        if pressed(Key::F5) || (ctrl && pressed(Key::R)) {
            self.actions.push(Action::Refresh);
        }
        if pressed(Key::F2) && !typing {
            self.actions.push(Action::BeginRename);
        }
        if mods.alt && pressed(Key::ArrowLeft) || (!ctrl && !typing && pressed(Key::Backspace)) {
            self.actions.push(Action::Back);
        }
        if mods.alt && pressed(Key::ArrowRight) {
            self.actions.push(Action::Forward);
        }
        if mods.alt && pressed(Key::ArrowUp) {
            self.actions.push(Action::Up);
        }
        if mods.alt && pressed(Key::Enter) {
            self.actions.push(Action::ShowProperties);
        }
        if ctrl && pressed(Key::T) {
            self.actions.push(Action::NewTab(None));
        }
        if ctrl && pressed(Key::W) {
            self.actions.push(Action::CloseTab(self.active));
        }
        if ctrl && pressed(Key::Tab) {
            let next = (self.active + 1) % self.tabs.len();
            self.actions.push(Action::SelectTab(next));
        }
        for (key, mode) in [
            (Key::Num1, ViewMode::Details),
            (Key::Num2, ViewMode::List),
            (Key::Num3, ViewMode::Icons),
            (Key::Num4, ViewMode::Tiles),
        ] {
            if ctrl && pressed(key) {
                self.actions.push(Action::SetViewMode(mode));
            }
        }
        if ctrl && !typing && pressed(Key::F) {
            self.focus_search = true;
        }
        if ctrl && !typing && pressed(Key::L) {
            self.path_edit = Some(self.tab().path().display().to_string());
        }
        if ctrl && !typing && pressed(Key::H) {
            self.actions.push(Action::ToggleHidden);
        }
        if ctrl && !typing && pressed(Key::A) {
            self.actions.push(Action::SelectAll);
        }
        if !typing && (clip.0 || (ctrl && pressed(Key::C))) {
            self.actions.push(Action::Copy);
        }
        if !typing && (clip.1 || (ctrl && pressed(Key::X))) {
            self.actions.push(Action::Cut);
        }
        if !typing && (clip.2 || (ctrl && pressed(Key::V))) {
            self.actions.push(Action::Paste);
        }
        if ctrl && pressed(Key::P) {
            self.actions.push(Action::TogglePreview);
        }
        if ctrl && pressed(Key::B) {
            self.actions.push(Action::ToggleSidebar);
        }
        if ctrl && mods.shift && pressed(Key::N) {
            self.actions.push(Action::NewFolder);
        }
        if pressed(Key::Delete) && !typing {
            self.actions.push(Action::DeleteSelected {
                permanent: mods.shift,
            });
        }
        if pressed(Key::Escape) && !typing {
            self.actions.push(Action::ClearSelection);
        }
        if pressed(Key::Enter) && !typing {
            let target = self.tab().selected_entries().next().map(|e| e.path.clone());
            if let Some(path) = target {
                self.actions.push(Action::Activate(path));
            }
        }

        if typing {
            return;
        }

        let grid = matches!(self.tab().view_mode, ViewMode::Icons | ViewMode::Tiles);
        let step = if grid { self.grid_columns(ctx) } else { 1 };
        let tab = self.tab_mut();
        if pressed(Key::ArrowDown) {
            tab.move_cursor(step as i64, mods.shift);
        }
        if pressed(Key::ArrowUp) {
            tab.move_cursor(-(step as i64), mods.shift);
        }
        if grid && pressed(Key::ArrowRight) {
            tab.move_cursor(1, mods.shift);
        }
        if grid && pressed(Key::ArrowLeft) {
            tab.move_cursor(-1, mods.shift);
        }
        if pressed(Key::Home) {
            tab.move_cursor(i64::MIN / 2, mods.shift);
        }
        if pressed(Key::End) {
            tab.move_cursor(i64::MAX / 2, mods.shift);
        }
        if pressed(Key::PageDown) {
            tab.move_cursor(step as i64 * 10, mods.shift);
        }
        if pressed(Key::PageUp) {
            tab.move_cursor(-(step as i64) * 10, mods.shift);
        }

        if !ctrl && !mods.alt {
            // Type-ahead, and +/- for icon size.
            let trimmed: String = typed.chars().filter(|c| !c.is_control()).collect();
            match trimmed.as_str() {
                "" => {}
                "+" | "=" => {
                    let next = (self.tab().icon_size * 1.25).min(256.0);
                    self.actions.push(Action::SetIconSize(next));
                }
                "-" => {
                    let next = (self.tab().icon_size / 1.25).max(32.0);
                    self.actions.push(Action::SetIconSize(next));
                }
                text => self.tab_mut().type_ahead(text),
            }
        }
    }

    /// Columns the grid currently shows, so arrow keys move by a visual row.
    fn grid_columns(&self, ctx: &Context) -> usize {
        let icon = self.tab().icon_size;
        let cell_w = if self.tab().view_mode == ViewMode::Tiles {
            icon * 2.6 + 24.0
        } else {
            icon + 20.0
        };
        let width = ctx.input(|i| i.content_rect()).width()
            - if self.settings.show_sidebar {
                self.settings.sidebar_width
            } else {
                0.0
            }
            - if self.settings.show_preview {
                self.settings.preview_width
            } else {
                0.0
            };
        (((width + 6.0) / (cell_w + 6.0)).floor() as usize).max(1)
    }

    // ------------------------------------------------------------------ chrome

    fn draw_toasts(&mut self, ctx: &Context, pal: &Palette) {
        self.toasts
            .retain(|t| t.born.elapsed() < Duration::from_secs(4));
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -40.0))
            .show(ctx, |ui| {
                for toast in self.toasts.iter().rev().take(4) {
                    let color = if toast.error {
                        egui::Color32::from_rgb(0xd0, 0x4a, 0x42)
                    } else {
                        pal.accent
                    };
                    egui::Frame::popup(ui.style())
                        .fill(ui.visuals().panel_fill)
                        .stroke(egui::Stroke::new(1.0, color))
                        .show(ui, |ui| {
                            ui.label(&toast.text);
                        });
                }
            });
        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn draw_progress(&mut self, ui: &mut egui::Ui) {
        let Some((label, done, total, current)) = self
            .running
            .as_ref()
            .map(|r| (r.label, r.done, r.total, r.current.clone()))
        else {
            return;
        };
        let mut cancel = false;
        ui.horizontal(|ui| {
            let frac = if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            };
            ui.label(label);
            ui.add(
                egui::ProgressBar::new(frac.clamp(0.0, 1.0))
                    .desired_width(200.0)
                    .show_percentage(),
            );
            ui.label(egui::RichText::new(current).small());
            cancel = ui.small_button("Cancel").clicked();
        });
        if cancel {
            self.ops.cancel_all();
            self.running = None;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root_ui.ctx().clone();
        if self.theme_applied != Some(self.settings.theme) {
            theme::apply(ctx, self.settings.theme, self.translucent);
            self.theme_applied = Some(self.settings.theme);
        }
        let pal = self.palette(ctx);

        self.thumbs.pump();
        self.collect_results();

        // Filesystem changes refresh the active tab, debounced by the watcher.
        if self.watcher.poll() {
            let active = self.active;
            if !self.tabs[active].filter.is_searching() {
                self.start_scan(active);
            }
        }
        // Volumes change when media is plugged in; poll rarely.
        if self.volumes_checked.elapsed() > Duration::from_secs(10) {
            self.places.volumes = places::volumes();
            self.volumes_checked = Instant::now();
        }

        // Re-filter and re-sort before anything draws: the status bar and the
        // listing must not disagree about what the tab contains.
        for tab in &mut self.tabs {
            if tab.needs_rebuild {
                tab.rebuild();
            }
        }

        self.keyboard(ctx);

        // --- top chrome: tabs, command bar, address bar (the Win11 order)
        let chrome_frame = egui::Frame::new()
            .fill(pal.chrome)
            .inner_margin(egui::Margin::symmetric(0, 2));
        egui::Panel::top("tabs")
            .frame(chrome_frame)
            .show_separator_line(false)
            .show(root_ui, |ui| {
                let tabs = &self.tabs;
                let active = self.active;
                let mut bars = Bars {
                    settings: &self.settings,
                    pal: &pal,
                    actions: &mut self.actions,
                    path_edit: &mut self.path_edit,
                    path_edit_fresh: &mut self.path_edit_fresh,
                    search: &mut self.search,
                    focus_search: self.focus_search,
                };
                toolbar::tab_strip(ui, tabs, active, &mut bars);
            });
        egui::Panel::top("commands")
            .frame(chrome_frame)
            .show_separator_line(false)
            .show(root_ui, |ui| {
                let tab = &self.tabs[self.active];
                let mut bars = Bars {
                    settings: &self.settings,
                    pal: &pal,
                    actions: &mut self.actions,
                    path_edit: &mut self.path_edit,
                    path_edit_fresh: &mut self.path_edit_fresh,
                    search: &mut self.search,
                    focus_search: self.focus_search,
                };
                toolbar::command_bar(ui, tab, &mut bars);
            });
        egui::Panel::top("address")
            .frame(
                egui::Frame::new()
                    .fill(pal.chrome)
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 2,
                        bottom: 6,
                    }),
            )
            .show_separator_line(false)
            .show(root_ui, |ui| {
                let tab = &self.tabs[self.active];
                let mut bars = Bars {
                    settings: &self.settings,
                    pal: &pal,
                    actions: &mut self.actions,
                    path_edit: &mut self.path_edit,
                    path_edit_fresh: &mut self.path_edit_fresh,
                    search: &mut self.search,
                    focus_search: self.focus_search,
                };
                toolbar::address_bar(ui, tab, &mut bars);
            });
        self.focus_search = false;

        // --- bottom chrome
        if self.settings.show_status_bar {
            egui::Panel::bottom("status")
                .frame(
                    egui::Frame::new()
                        .fill(pal.surface)
                        .inner_margin(egui::Margin::symmetric(10, 3)),
                )
                .show_separator_line(false)
                .show(root_ui, |ui| {
                    self.draw_progress(ui);
                    statusbar::show(
                        ui,
                        &self.tabs[self.active],
                        &self.places,
                        &pal,
                        &mut self.actions,
                    );
                });
        }

        // --- side panels
        if self.settings.show_sidebar {
            let mut width = self.settings.sidebar_width;
            egui::Panel::left("sidebar")
                .resizable(true)
                .default_size(width)
                .size_range(140.0..=420.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.surface)
                        .inner_margin(egui::Margin::symmetric(6, 4)),
                )
                .show_separator_line(false)
                .show(root_ui, |ui| {
                    width = ui.available_width();
                    let current = self.tabs[self.active].path().to_path_buf();
                    sidebar::show(
                        ui,
                        &self.places,
                        &current,
                        &mut self.sidebar,
                        &pal,
                        &mut self.actions,
                    );
                });
            self.settings.sidebar_width = width;
        }

        if self.settings.show_preview {
            let selected = self.tabs[self.active]
                .selected_entries()
                .next()
                .cloned()
                .or_else(|| {
                    self.tabs[self.active]
                        .cursor
                        .and_then(|c| self.tabs[self.active].entry_at(c).cloned())
                });
            let mut width = self.settings.preview_width;
            egui::Panel::right("preview")
                .resizable(true)
                .default_size(width)
                .size_range(180.0..=560.0)
                .frame(
                    egui::Frame::new()
                        .fill(pal.surface)
                        .inner_margin(egui::Margin::symmetric(10, 6)),
                )
                .show(root_ui, |ui| {
                    width = ui.available_width();
                    let mut view_ctx = ViewCtx {
                        settings: &self.settings,
                        pal: &pal,
                        thumbs: &self.thumbs,
                        textures: &mut self.textures,
                        clipboard: &self.clipboard,
                        actions: &mut self.actions,
                    };
                    crate::preview::show(ui, selected.as_ref(), &mut view_ctx);
                });
            self.settings.preview_width = width;
        }

        // --- listing
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal.surface)
                    .inner_margin(egui::Margin::symmetric(6, 2)),
            )
            .show(root_ui, |ui| {
                let tab = &mut self.tabs[self.active];
                let mut view_ctx = ViewCtx {
                    settings: &self.settings,
                    pal: &pal,
                    thumbs: &self.thumbs,
                    textures: &mut self.textures,
                    clipboard: &self.clipboard,
                    actions: &mut self.actions,
                };
                views::show(ui, tab, &mut view_ctx);
            });

        // --- overlays
        if let Some(mut dialog) = self.dialog.take() {
            match dialogs::show(ctx, &mut dialog, &pal) {
                DialogOutcome::None => self.dialog = Some(dialog),
                DialogOutcome::Close => {}
                DialogOutcome::Submit(action) => {
                    self.dialog = Some(dialog);
                    self.actions.push(action);
                }
            }
        }
        self.draw_toasts(ctx, &pal);

        // Files dropped from another application.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if !dropped.is_empty() {
            let dir = self.tabs[self.active].path().to_path_buf();
            self.actions.push(Action::DropOnto {
                paths: dropped,
                dir,
                copy: true,
            });
        }

        // Run the queued actions once the frame's borrows are gone.
        let queued = std::mem::take(&mut self.actions);
        for action in queued {
            self.apply(action, ctx);
        }

        self.textures.trim(512);
        if self.thumbs.inflight() > 0 || self.watcher.pending() || self.running.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }

        let title = format!("{} — Explorer", self.tabs[self.active].title());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// With a backdrop the window must be cleared to nothing, so Mica or the
    /// desktop shows through; otherwise clear to the chrome colour.
    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        if self.translucent {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            visuals.panel_fill.to_normalized_gamma_f32()
        }
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

impl App {
    fn persist(&mut self) {
        self.settings.last_tabs = self.tabs.iter().map(|t| t.path().to_path_buf()).collect();
        self.settings.save();
    }
}

/// Follow a file argument up to its folder, and resolve `.` / `..`.
fn resolve_dir(path: &std::path::Path) -> PathBuf {
    let p = if path.is_file() {
        path.parent().unwrap_or(path).to_path_buf()
    } else {
        path.to_path_buf()
    };
    std::fs::canonicalize(&p).unwrap_or(p)
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}
