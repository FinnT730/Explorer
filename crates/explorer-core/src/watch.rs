use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

/// Watches the directory a tab is showing and reports that it changed.
///
/// Events are coalesced: a burst of writes produces one refresh, which keeps a
/// busy download folder from re-scanning on every chunk.
pub struct DirWatcher {
    watcher: Option<RecommendedWatcher>,
    rx: Receiver<notify::Result<Event>>,
    watched: Option<PathBuf>,
    dirty_since: Option<Instant>,
    debounce: Duration,
}

impl DirWatcher {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (tx, rx) = channel();
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
                wake();
            },
            Config::default(),
        )
        .ok();
        Self {
            watcher,
            rx,
            watched: None,
            dirty_since: None,
            debounce: Duration::from_millis(250),
        }
    }

    /// Point the watcher at `path`, replacing any previous target.
    pub fn watch(&mut self, path: &Path) {
        if self.watched.as_deref() == Some(path) {
            return;
        }
        let Some(w) = self.watcher.as_mut() else {
            return;
        };
        if let Some(old) = self.watched.take() {
            let _ = w.unwatch(&old);
        }
        if w.watch(path, RecursiveMode::NonRecursive).is_ok() {
            self.watched = Some(path.to_path_buf());
        }
        self.dirty_since = None;
    }

    pub fn watched(&self) -> Option<&Path> {
        self.watched.as_deref()
    }

    /// Returns true once per settled burst of filesystem changes.
    pub fn poll(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(Ok(ev)) if is_interesting(&ev) => self.dirty_since = Some(Instant::now()),
                Ok(_) => {}
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        match self.dirty_since {
            Some(t) if t.elapsed() >= self.debounce => {
                self.dirty_since = None;
                true
            }
            _ => false,
        }
    }

    /// Whether a refresh is pending, so the UI can keep animating.
    pub fn pending(&self) -> bool {
        self.dirty_since.is_some()
    }
}

fn is_interesting(ev: &Event) -> bool {
    use notify::EventKind;
    matches!(
        ev.kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_) | EventKind::Any
    )
}
