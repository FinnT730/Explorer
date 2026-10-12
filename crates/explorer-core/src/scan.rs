use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use rayon::prelude::*;

use crate::entry::Entry;
use crate::filter::matches_query;

/// Work handed to the scanner thread.
enum Job {
    List {
        id: u64,
        path: PathBuf,
        cancel: Arc<AtomicBool>,
    },
    Search {
        id: u64,
        root: PathBuf,
        query: String,
        cancel: Arc<AtomicBool>,
    },
    Measure {
        id: u64,
        path: PathBuf,
        cancel: Arc<AtomicBool>,
    },
}

/// Something the scanner produced. Every variant carries the request id so the
/// UI can drop results that belong to a navigation it has already left.
#[derive(Debug)]
pub enum ScanResult {
    Listing {
        id: u64,
        path: PathBuf,
        entries: Vec<Entry>,
    },
    /// One batch of recursive-search hits; `done` marks the final batch.
    SearchBatch {
        id: u64,
        entries: Vec<Entry>,
        done: bool,
    },
    Measured {
        id: u64,
        path: PathBuf,
        bytes: u64,
        files: usize,
        dirs: usize,
    },
    Failed {
        id: u64,
        path: PathBuf,
        error: String,
    },
}

impl ScanResult {
    pub fn id(&self) -> u64 {
        match self {
            ScanResult::Listing { id, .. }
            | ScanResult::SearchBatch { id, .. }
            | ScanResult::Measured { id, .. }
            | ScanResult::Failed { id, .. } => *id,
        }
    }
}

/// Off-thread directory scanning, recursive search and folder measurement.
///
/// Keeping this off the UI thread is what makes navigation into a huge
/// directory feel instant: the frame never blocks on `read_dir`.
pub struct Scanner {
    jobs: Sender<Job>,
    results: Receiver<ScanResult>,
    next_id: u64,
    cancel: Arc<AtomicBool>,
    /// Woken by the worker so the UI can repaint as results land.
    on_result: Arc<dyn Fn() + Send + Sync>,
}

impl Scanner {
    pub fn new(on_result: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (jtx, jrx) = channel::<Job>();
        let (rtx, rrx) = channel::<ScanResult>();
        let wake = on_result.clone();
        std::thread::Builder::new()
            .name("explorer-scanner".into())
            .spawn(move || worker(jrx, rtx, wake))
            .expect("spawn scanner thread");
        Self {
            jobs: jtx,
            results: rrx,
            next_id: 1,
            cancel: Arc::new(AtomicBool::new(false)),
            on_result,
        }
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Cancel every in-flight job. Call this before issuing a new listing for
    /// the same tab so a slow network share cannot hold up the next one.
    pub fn cancel_all(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
    }

    pub fn list(&mut self, path: impl Into<PathBuf>) -> u64 {
        let id = self.take_id();
        let _ = self.jobs.send(Job::List {
            id,
            path: path.into(),
            cancel: self.cancel.clone(),
        });
        id
    }

    pub fn search(&mut self, root: impl Into<PathBuf>, query: impl Into<String>) -> u64 {
        let id = self.take_id();
        let _ = self.jobs.send(Job::Search {
            id,
            root: root.into(),
            query: query.into(),
            cancel: self.cancel.clone(),
        });
        id
    }

    pub fn measure(&mut self, path: impl Into<PathBuf>) -> u64 {
        let id = self.take_id();
        let _ = self.jobs.send(Job::Measure {
            id,
            path: path.into(),
            cancel: self.cancel.clone(),
        });
        id
    }

    /// Drain everything the worker has finished since the last frame.
    pub fn drain(&self) -> Vec<ScanResult> {
        let mut out = Vec::new();
        while let Ok(r) = self.results.try_recv() {
            out.push(r);
        }
        out
    }
}

impl std::fmt::Debug for Scanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scanner")
            .field("next_id", &self.next_id)
            .finish_non_exhaustive()
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        let _ = &self.on_result;
    }
}

fn worker(jobs: Receiver<Job>, out: Sender<ScanResult>, wake: Arc<dyn Fn() + Send + Sync>) {
    while let Ok(job) = jobs.recv() {
        match job {
            Job::List { id, path, cancel } => {
                if cancel.load(Ordering::Relaxed) {
                    continue;
                }
                let res = match read_dir(&path) {
                    Ok(entries) => ScanResult::Listing { id, path, entries },
                    Err(e) => ScanResult::Failed {
                        id,
                        path,
                        error: e.to_string(),
                    },
                };
                if out.send(res).is_err() {
                    return;
                }
            }
            Job::Search {
                id,
                root,
                query,
                cancel,
            } => {
                if search_tree(id, &root, &query, &cancel, &out, &wake).is_err() {
                    return;
                }
                continue;
            }
            Job::Measure { id, path, cancel } => {
                let (bytes, files, dirs) = measure_tree(&path, &cancel);
                if out
                    .send(ScanResult::Measured {
                        id,
                        path,
                        bytes,
                        files,
                        dirs,
                    })
                    .is_err()
                {
                    return;
                }
            }
        }
        wake();
    }
}

/// Read one directory into entries. Metadata is gathered in parallel because
/// `stat` dominates the cost on large directories.
pub fn read_dir(path: &Path) -> std::io::Result<Vec<Entry>> {
    let raw: Vec<_> = std::fs::read_dir(path)?.filter_map(Result::ok).collect();
    let mut entries: Vec<Entry> = raw
        .into_par_iter()
        .filter_map(|de| {
            let p = de.path();
            match de.metadata() {
                Ok(m) => Some(Entry::from_meta(p, &m)),
                Err(_) => Entry::from_path(p).ok(),
            }
        })
        .collect();
    entries.shrink_to_fit();
    Ok(entries)
}

fn search_tree(
    id: u64,
    root: &Path,
    query: &str,
    cancel: &AtomicBool,
    out: &Sender<ScanResult>,
    wake: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(), ()> {
    let needle = query.to_lowercase();
    let mut batch: Vec<Entry> = Vec::with_capacity(128);
    let walker = walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(32)
        .into_iter()
        .filter_entry(|e| !cancel.load(Ordering::Relaxed) || e.depth() == 0);

    for de in walker.filter_map(Result::ok) {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        if de.depth() == 0 {
            continue;
        }
        let name = de.file_name().to_string_lossy().to_lowercase();
        if !matches_query(&name, &needle) {
            continue;
        }
        if let Ok(meta) = de.metadata() {
            batch.push(Entry::from_meta(de.path().to_path_buf(), &meta));
        }
        // Stream partial results so hits appear while the walk continues.
        if batch.len() >= 128 {
            out.send(ScanResult::SearchBatch {
                id,
                entries: std::mem::take(&mut batch),
                done: false,
            })
            .map_err(|_| ())?;
            wake();
            batch = Vec::with_capacity(128);
        }
    }
    out.send(ScanResult::SearchBatch {
        id,
        entries: batch,
        done: true,
    })
    .map_err(|_| ())?;
    wake();
    Ok(())
}

/// Recursive size of a directory: bytes, file count, subdirectory count.
pub fn measure_tree(path: &Path, cancel: &AtomicBool) -> (u64, usize, usize) {
    let mut bytes = 0u64;
    let mut files = 0usize;
    let mut dirs = 0usize;
    for de in walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if de.depth() == 0 {
            continue;
        }
        match de.metadata() {
            Ok(m) if m.is_dir() => dirs += 1,
            Ok(m) => {
                files += 1;
                bytes += m.len();
            }
            Err(_) => {}
        }
    }
    (bytes, files, dirs)
}
