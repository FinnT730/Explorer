use std::path::{Path, PathBuf};

use sysinfo::Disks;

/// Sidebar section a place belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaceKind {
    Quick,
    Volume,
    Pinned,
}

#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    pub path: PathBuf,
    pub kind: PlaceKind,
}

/// A mounted volume, with the capacity readout the status bar shows.
#[derive(Clone, Debug)]
pub struct Volume {
    pub name: String,
    pub mount: PathBuf,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
    pub fs: String,
}

impl Volume {
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    pub fn used_fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used() as f32 / self.total as f32
        }
    }
}

/// The sidebar's contents: known user folders, mounted volumes and user pins.
#[derive(Debug, Default)]
pub struct Places {
    pub quick: Vec<Place>,
    pub volumes: Vec<Volume>,
    pub pinned: Vec<Place>,
}

impl Places {
    pub fn load(pinned: Vec<PathBuf>) -> Self {
        let mut quick = Vec::new();
        let mut add = |name: &str, path: Option<PathBuf>| {
            if let Some(p) = path {
                if p.is_dir() {
                    quick.push(Place {
                        name: name.to_string(),
                        path: p,
                        kind: PlaceKind::Quick,
                    });
                }
            }
        };
        add("Home", dirs::home_dir());
        add("Desktop", dirs::desktop_dir());
        add("Documents", dirs::document_dir());
        add("Downloads", dirs::download_dir());
        add("Pictures", dirs::picture_dir());
        add("Music", dirs::audio_dir());
        add("Videos", dirs::video_dir());

        Self {
            quick,
            volumes: volumes(),
            pinned: pinned
                .into_iter()
                .filter(|p| p.is_dir())
                .map(|p| Place {
                    name: crate::entry::file_name_of(&p),
                    path: p,
                    kind: PlaceKind::Pinned,
                })
                .collect(),
        }
    }

    /// Volume containing `path`, chosen by longest matching mount point.
    pub fn volume_for(&self, path: &Path) -> Option<&Volume> {
        self.volumes
            .iter()
            .filter(|v| path.starts_with(&v.mount))
            .max_by_key(|v| v.mount.as_os_str().len())
    }
}

pub fn volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<Volume> = disks
        .list()
        .iter()
        .map(|d| {
            let mount = d.mount_point().to_path_buf();
            let label = d.name().to_string_lossy().to_string();
            let name = if label.trim().is_empty() {
                mount.to_string_lossy().to_string()
            } else {
                format!("{label} ({})", mount.display())
            };
            Volume {
                name,
                mount,
                total: d.total_space(),
                available: d.available_space(),
                removable: d.is_removable(),
                fs: d.file_system().to_string_lossy().to_string(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

/// Where a new window starts: the user's home, or the filesystem root.
pub fn default_start_dir() -> PathBuf {
    dirs::home_dir()
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" }))
}
