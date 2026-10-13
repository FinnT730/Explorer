//! Window-level preferences, persisted to the user's config directory.

use std::path::PathBuf;

use explorer_core::{SortDir, SortKey, SortSpec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ViewMode {
    /// Multi-column table with sortable headers.
    Details,
    /// Compact single-line rows, wrapping into columns.
    List,
    /// Icon grid; the size slider turns this into small/medium/large/extra large.
    Icons,
    /// Icon plus name and metadata beside it.
    Tiles,
}

impl ViewMode {
    pub const ALL: [ViewMode; 4] = [
        ViewMode::Details,
        ViewMode::List,
        ViewMode::Icons,
        ViewMode::Tiles,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Details => "Details",
            ViewMode::List => "List",
            ViewMode::Icons => "Icons",
            ViewMode::Tiles => "Tiles",
        }
    }
}

/// Window background effect. Mica and acrylic are Windows 11 features; on
/// macOS they map to the closest vibrancy material, and elsewhere the window
/// stays opaque.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Backdrop {
    /// Opaque window.
    None,
    /// Mica: the desktop wallpaper, blurred and tinted. Win11 22000+.
    Mica,
    /// Mica Alt, the "tabbed" variant Explorer itself uses. Win11 22523+.
    Tabbed,
    /// Acrylic: blurs whatever is behind the window.
    Acrylic,
}

impl Backdrop {
    pub const ALL: [Backdrop; 4] = [
        Backdrop::None,
        Backdrop::Mica,
        Backdrop::Tabbed,
        Backdrop::Acrylic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Backdrop::None => "None (opaque)",
            Backdrop::Mica => "Mica",
            Backdrop::Tabbed => "Mica Alt",
            Backdrop::Acrylic => "Acrylic",
        }
    }

    /// Whether the window must be created transparent for this to show.
    pub fn needs_transparency(self) -> bool {
        self != Backdrop::None
    }

    /// The default: Explorer's own look on Windows 11, opaque elsewhere.
    pub fn platform_default() -> Self {
        if cfg!(windows) {
            Backdrop::Tabbed
        } else {
            Backdrop::None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Theme {
    Dark,
    Light,
    System,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub backdrop: Backdrop,
    pub view_mode: ViewMode,
    pub icon_size: f32,
    pub show_hidden: bool,
    pub show_sidebar: bool,
    pub show_preview: bool,
    pub show_status_bar: bool,
    pub sidebar_width: f32,
    pub preview_width: f32,
    pub sort: SortSpec,
    pub dirs_first: bool,
    /// Columns visible in Details view.
    pub columns: Columns,
    pub pinned: Vec<PathBuf>,
    /// Restored on next launch.
    pub last_tabs: Vec<PathBuf>,
    pub confirm_delete: bool,
    /// Double-click to open (Explorer default) vs single click.
    pub single_click_open: bool,
    pub thumbnails: bool,
    pub thumbnail_cache: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Columns {
    pub modified: bool,
    pub kind: bool,
    pub size: bool,
    pub created: bool,
    pub extension: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            modified: true,
            kind: true,
            size: true,
            created: false,
            extension: false,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            backdrop: Backdrop::platform_default(),
            view_mode: ViewMode::Details,
            icon_size: 96.0,
            show_hidden: false,
            show_sidebar: true,
            show_preview: false,
            show_status_bar: true,
            sidebar_width: 220.0,
            preview_width: 300.0,
            sort: SortSpec {
                key: SortKey::Name,
                dir: SortDir::Asc,
                dirs_first: true,
            },
            dirs_first: true,
            columns: Columns::default(),
            pinned: Vec::new(),
            last_tabs: Vec::new(),
            confirm_delete: true,
            single_click_open: false,
            thumbnails: true,
            thumbnail_cache: 2048,
        }
    }
}

impl Settings {
    pub fn path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("explorer").join("settings.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }
}
