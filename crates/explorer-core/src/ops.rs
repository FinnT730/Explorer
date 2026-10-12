//! File operations: clipboard, copy/move/delete/rename, and launching files.
//!
//! Long-running work (copying a tree, emptying to the recycle bin) runs on a
//! worker thread and reports progress, so the window never freezes mid-copy.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipMode {
    Copy,
    Cut,
}

/// Explorer-style clipboard: holds paths plus whether they were cut or copied.
#[derive(Clone, Debug, Default)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub mode: Option<ClipMode>,
}

impl Clipboard {
    pub fn set(&mut self, paths: Vec<PathBuf>, mode: ClipMode) {
        self.paths = paths;
        self.mode = Some(mode);
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.mode = None;
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() || self.mode.is_none()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.paths.iter().any(|p| p == path)
    }
}

#[derive(Clone, Debug)]
pub enum Task {
    Copy {
        srcs: Vec<PathBuf>,
        dst_dir: PathBuf,
    },
    Move {
        srcs: Vec<PathBuf>,
        dst_dir: PathBuf,
    },
    Trash(Vec<PathBuf>),
    Delete(Vec<PathBuf>),
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    NewFolder(PathBuf),
    NewFile(PathBuf),
}

impl Task {
    pub fn label(&self) -> &'static str {
        match self {
            Task::Copy { .. } => "Copying",
            Task::Move { .. } => "Moving",
            Task::Trash(_) => "Moving to recycle bin",
            Task::Delete(_) => "Deleting",
            Task::Rename { .. } => "Renaming",
            Task::NewFolder(_) => "Creating folder",
            Task::NewFile(_) => "Creating file",
        }
    }

    /// Directories the operation changes, so affected tabs can refresh.
    pub fn touched_dirs(&self) -> Vec<PathBuf> {
        let parents = |ps: &Vec<PathBuf>| {
            ps.iter()
                .filter_map(|p| p.parent().map(Path::to_path_buf))
                .collect::<Vec<_>>()
        };
        match self {
            Task::Copy { srcs, dst_dir } => {
                let mut v = parents(srcs);
                v.push(dst_dir.clone());
                v
            }
            Task::Move { srcs, dst_dir } => {
                let mut v = parents(srcs);
                v.push(dst_dir.clone());
                v
            }
            Task::Trash(ps) | Task::Delete(ps) => parents(ps),
            Task::Rename { from, to } => parents(&vec![from.clone(), to.clone()]),
            Task::NewFolder(p) | Task::NewFile(p) => p
                .parent()
                .map(|q| vec![q.to_path_buf()])
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug)]
pub enum Progress {
    Started {
        id: u64,
        label: &'static str,
        total: u64,
    },
    Advanced {
        id: u64,
        done: u64,
        total: u64,
        current: PathBuf,
    },
    Finished {
        id: u64,
        task: Task,
        errors: Vec<String>,
    },
}

/// Handle to the file-operation worker thread.
pub struct Ops {
    tx: Sender<(u64, Task, Arc<AtomicBool>)>,
    rx: Receiver<Progress>,
    next_id: u64,
    cancel: Arc<AtomicBool>,
}

impl Ops {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (ttx, trx) = channel::<(u64, Task, Arc<AtomicBool>)>();
        let (ptx, prx) = channel::<Progress>();
        std::thread::Builder::new()
            .name("explorer-ops".into())
            .spawn(move || {
                while let Ok((id, task, cancel)) = trx.recv() {
                    let errors = run_task(id, &task, &cancel, &ptx);
                    if ptx.send(Progress::Finished { id, task, errors }).is_err() {
                        return;
                    }
                    wake();
                }
            })
            .expect("spawn ops thread");
        Self {
            tx: ttx,
            rx: prx,
            next_id: 1,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn submit(&mut self, task: Task) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let _ = self.tx.send((id, task, self.cancel.clone()));
        id
    }

    pub fn cancel_all(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
    }

    pub fn drain(&self) -> Vec<Progress> {
        let mut out = Vec::new();
        while let Ok(p) = self.rx.try_recv() {
            out.push(p);
        }
        out
    }
}

fn run_task(id: u64, task: &Task, cancel: &AtomicBool, out: &Sender<Progress>) -> Vec<String> {
    let mut errors = Vec::new();
    match task {
        Task::Copy { srcs, dst_dir } | Task::Move { srcs, dst_dir } => {
            let moving = matches!(task, Task::Move { .. });
            let total: u64 = srcs.iter().map(|p| weight(p)).sum();
            let _ = out.send(Progress::Started {
                id,
                label: task.label(),
                total,
            });
            let mut done = 0u64;
            for src in srcs {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let dst = unique_target(dst_dir, src);
                let r = if moving {
                    move_path(src, &dst)
                } else {
                    copy_path(src, &dst, cancel, &mut |p| {
                        let _ = out.send(Progress::Advanced {
                            id,
                            done,
                            total,
                            current: p.to_path_buf(),
                        });
                    })
                };
                if let Err(e) = r {
                    errors.push(format!("{}: {e}", src.display()));
                }
                done += weight(src);
                let _ = out.send(Progress::Advanced {
                    id,
                    done,
                    total,
                    current: src.clone(),
                });
            }
        }
        Task::Trash(paths) => {
            let _ = out.send(Progress::Started {
                id,
                label: task.label(),
                total: paths.len() as u64,
            });
            if let Err(e) = trash::delete_all(paths) {
                // Some targets (network shares, tmpfs) have no recycle bin.
                errors.push(e.to_string());
            }
        }
        Task::Delete(paths) => {
            let _ = out.send(Progress::Started {
                id,
                label: task.label(),
                total: paths.len() as u64,
            });
            for (i, p) in paths.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                let r = if p.is_dir() && !p.is_symlink() {
                    std::fs::remove_dir_all(p)
                } else {
                    std::fs::remove_file(p)
                };
                if let Err(e) = r {
                    errors.push(format!("{}: {e}", p.display()));
                }
                let _ = out.send(Progress::Advanced {
                    id,
                    done: i as u64 + 1,
                    total: paths.len() as u64,
                    current: p.clone(),
                });
            }
        }
        Task::Rename { from, to } => {
            if to.exists() {
                errors.push(format!("{} already exists", to.display()));
            } else if let Err(e) = std::fs::rename(from, to) {
                errors.push(format!("{}: {e}", from.display()));
            }
        }
        Task::NewFolder(p) => {
            if let Err(e) = std::fs::create_dir(p) {
                errors.push(format!("{}: {e}", p.display()));
            }
        }
        Task::NewFile(p) => {
            if p.exists() {
                errors.push(format!("{} already exists", p.display()));
            } else if let Err(e) = std::fs::File::create(p) {
                errors.push(format!("{}: {e}", p.display()));
            }
        }
    }
    errors
}

/// Cheap progress weight: file size, or 1 per directory entry.
fn weight(path: &Path) -> u64 {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => 1,
        Ok(m) => m.len().max(1),
        Err(_) => 1,
    }
}

/// Pick a destination name, adding ` (2)`, ` (3)`, … on collision.
pub fn unique_target(dst_dir: &Path, src: &Path) -> PathBuf {
    let name = crate::entry::file_name_of(src);
    let candidate = dst_dir.join(&name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = split_name(&name);
    for n in 2..10_000 {
        let next = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let p = dst_dir.join(next);
        if !p.exists() {
            return p;
        }
    }
    candidate
}

pub fn split_name(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        // A leading dot is part of the name (`.gitignore`), not an extension.
        Some(i) if i > 0 => (&name[..i], &name[i + 1..]),
        _ => (name, ""),
    }
}

fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    match std::fs::rename(src, dst) {
        Ok(()) => Ok(()),
        // Across filesystems rename fails; fall back to copy + delete.
        Err(_) => {
            let cancel = AtomicBool::new(false);
            copy_path(src, dst, &cancel, &mut |_| {})?;
            if src.is_dir() && !src.is_symlink() {
                std::fs::remove_dir_all(src)
            } else {
                std::fs::remove_file(src)
            }
        }
    }
}

fn copy_path(
    src: &Path,
    dst: &Path,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(&Path),
) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(src)?;
    if meta.is_dir() {
        std::fs::create_dir_all(dst)?;
        for de in std::fs::read_dir(src)?.filter_map(Result::ok) {
            if cancel.load(Ordering::Relaxed) {
                return Ok(());
            }
            let child_dst = dst.join(de.file_name());
            copy_path(&de.path(), &child_dst, cancel, report)?;
        }
        Ok(())
    } else {
        report(src);
        std::fs::copy(src, dst).map(|_| ())
    }
}

/// Open a path with the OS default handler.
pub fn open_default(path: &Path) -> std::io::Result<()> {
    open::that_detached(path)
}

/// Reveal a path in the platform's own file manager.
pub fn reveal(path: &Path) -> std::io::Result<()> {
    let target = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent().unwrap_or(path).to_path_buf()
    };
    open::that_detached(target)
}

/// Open a terminal in `dir`, trying the usual suspects per platform.
pub fn open_terminal(dir: &Path) -> std::io::Result<()> {
    use std::process::Command;
    #[cfg(windows)]
    let candidates: [(&str, Vec<String>); 2] = [
        ("wt.exe", vec!["-d".into(), dir.display().to_string()]),
        (
            "cmd.exe",
            vec!["/K".into(), format!("cd /d {}", dir.display())],
        ),
    ];
    #[cfg(target_os = "macos")]
    let candidates: [(&str, Vec<String>); 1] = [(
        "open",
        vec!["-a".into(), "Terminal".into(), dir.display().to_string()],
    )];
    #[cfg(all(unix, not(target_os = "macos")))]
    let candidates: [(&str, Vec<String>); 5] = [
        ("x-terminal-emulator", vec![]),
        ("gnome-terminal", vec![]),
        ("konsole", vec![]),
        ("alacritty", vec![]),
        ("xterm", vec![]),
    ];

    for (bin, args) in candidates {
        let mut cmd = Command::new(bin);
        cmd.args(&args).current_dir(dir);
        if cmd.spawn().is_ok() {
            return Ok(());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no terminal emulator found",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_split_keeps_dotfiles_whole() {
        assert_eq!(split_name("a.txt"), ("a", "txt"));
        assert_eq!(split_name(".gitignore"), (".gitignore", ""));
        assert_eq!(split_name("archive.tar.gz"), ("archive.tar", "gz"));
    }

    #[test]
    fn collisions_get_numbered() {
        let dir = std::env::temp_dir().join(format!("explorer-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.txt");
        std::fs::write(&src, b"x").unwrap();
        assert_eq!(unique_target(&dir, &src), dir.join("a (2).txt"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
