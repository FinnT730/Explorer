//! Everything the UI can ask the app to do.
//!
//! Widgets queue actions instead of mutating app state directly: egui draws
//! inside closures that already borrow the tab, so deferring the mutation to
//! the end of the frame keeps the borrow checker (and the code) simple.

use std::path::PathBuf;

use explorer_core::{SortKey, Volume};

use crate::settings::{Backdrop, Theme, ViewMode};

#[derive(Clone, Debug)]
pub enum Action {
    // Navigation
    Navigate(PathBuf),
    OpenInNewTab(PathBuf),
    Activate(PathBuf),
    Back,
    Forward,
    Up,
    Refresh,

    // Tabs
    NewTab(Option<PathBuf>),
    CloseTab(usize),
    SelectTab(usize),
    DuplicateTab(usize),

    // View
    SetViewMode(ViewMode),
    SetIconSize(f32),
    SortBy(SortKey),
    ToggleDirsFirst,
    ToggleHidden,
    ToggleSidebar,
    TogglePreview,
    ToggleStatusBar,
    ToggleColumn(Column),
    SetTheme(Theme),
    SetBackdrop(Backdrop),

    // Selection
    SelectAll,
    InvertSelection,
    ClearSelection,
    SelectIndex {
        index: usize,
        extend: bool,
        toggle: bool,
    },

    // Search
    SetQuery(String),
    SearchHere,
    ClearSearch,

    // Clipboard and file operations
    Copy,
    Cut,
    Paste,
    DropOnto {
        paths: Vec<PathBuf>,
        dir: PathBuf,
        copy: bool,
    },
    DeleteSelected {
        permanent: bool,
    },
    BeginRename,
    CommitRename {
        from: PathBuf,
        to_name: String,
    },
    CancelRename,
    NewFolder,
    NewFile,
    CopyPath,
    OpenTerminal,
    RevealInOsFileManager,
    ShowProperties,

    // Sidebar
    Pin(PathBuf),
    Unpin(PathBuf),
    RefreshVolumes,

    // Dialogs
    ShowShortcuts,
    Toast(String),
    CloseDialog,
    Confirm,
    Dismiss(usize),
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Column {
    Size,
    Kind,
    Modified,
    Created,
    Extension,
}

impl Column {
    pub const ALL: [Column; 5] = [
        Column::Size,
        Column::Kind,
        Column::Modified,
        Column::Created,
        Column::Extension,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Column::Size => "Size",
            Column::Kind => "Kind",
            Column::Modified => "Date modified",
            Column::Created => "Date created",
            Column::Extension => "Type",
        }
    }

    pub fn sort_key(self) -> SortKey {
        match self {
            Column::Size => SortKey::Size,
            Column::Kind => SortKey::Kind,
            Column::Modified => SortKey::Modified,
            Column::Created => SortKey::Created,
            Column::Extension => SortKey::Extension,
        }
    }
}

/// Volume row the sidebar hands back for a tooltip, kept here so the sidebar
/// stays free of app state.
pub type VolumeRef<'a> = &'a Volume;
