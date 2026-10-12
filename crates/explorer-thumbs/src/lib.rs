//! Background image thumbnailing with a bounded in-memory cache.
//!
//! The UI asks for a thumbnail every frame it draws a tile. The first ask
//! queues a decode on a worker pool; later asks hit the cache. Nothing blocks
//! the UI thread, and the cache is capped so browsing a folder of 50k photos
//! cannot exhaust memory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::Mutex;

/// A decoded RGBA thumbnail, ready to upload as a texture.
#[derive(Clone)]
pub struct Thumb {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<Vec<u8>>,
    /// Pixel size of the source image, shown in the preview pane.
    pub source: (u32, u32),
}

impl std::fmt::Debug for Thumb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumb")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("source", &self.source)
            .finish()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Key {
    path_hash: u64,
    mtime: u64,
    bucket: u32,
}

#[derive(Clone, Debug)]
pub enum State {
    /// Not requested yet, or evicted.
    Missing,
    Pending,
    Ready(Thumb),
    Failed,
}

struct Job {
    key: Key,
    path: PathBuf,
    size: u32,
    generation: u64,
}

/// Thumbnail cache plus its decoder pool.
pub struct Thumbnails {
    cache: Mutex<HashMap<Key, State>>,
    /// Insertion order, used for FIFO eviction once `capacity` is exceeded.
    order: Mutex<Vec<Key>>,
    capacity: usize,
    jobs: Sender<Job>,
    done: Receiver<(Key, State)>,
    generation: Arc<AtomicU64>,
    inflight: Mutex<usize>,
}

impl Thumbnails {
    /// `workers` decode threads; `capacity` thumbnails kept in memory.
    pub fn new(workers: usize, capacity: usize, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let (jtx, jrx) = channel::<Job>();
        let jrx = Arc::new(Mutex::new(jrx));
        let (dtx, drx) = channel::<(Key, State)>();
        let generation = Arc::new(AtomicU64::new(0));

        for i in 0..workers.max(1) {
            let jrx = jrx.clone();
            let dtx: Sender<(Key, State)> = dtx.clone();
            let wake = wake.clone();
            let generation = generation.clone();
            std::thread::Builder::new()
                .name(format!("explorer-thumb-{i}"))
                .spawn(move || loop {
                    let job = {
                        let guard = jrx.lock();
                        guard.recv()
                    };
                    let Ok(job) = job else { return };
                    // Skip work for a directory the user already left.
                    if job.generation != generation.load(Ordering::Relaxed) {
                        let _ = dtx.send((job.key, State::Missing));
                        continue;
                    }
                    let state = match decode(&job.path, job.size) {
                        Some(t) => State::Ready(t),
                        None => State::Failed,
                    };
                    if dtx.send((job.key, state)).is_err() {
                        return;
                    }
                    wake();
                })
                .expect("spawn thumbnail worker");
        }

        Self {
            cache: Mutex::new(HashMap::new()),
            order: Mutex::new(Vec::new()),
            capacity,
            jobs: jtx,
            done: drx,
            generation,
            inflight: Mutex::new(0),
        }
    }

    /// Mark previously queued work as stale; call on navigation.
    pub fn new_generation(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Move finished decodes into the cache. Call once per frame.
    pub fn pump(&self) {
        let mut cache = self.cache.lock();
        let mut order = self.order.lock();
        let mut inflight = self.inflight.lock();
        while let Ok((key, state)) = self.done.try_recv() {
            *inflight = inflight.saturating_sub(1);
            match state {
                State::Missing => {
                    cache.remove(&key);
                    order.retain(|k| *k != key);
                }
                other => {
                    if cache.insert(key, other).is_none() {
                        order.push(key);
                    }
                }
            }
        }
        // FIFO eviction keeps memory bounded without per-frame bookkeeping.
        while order.len() > self.capacity {
            let key = order.remove(0);
            cache.remove(&key);
        }
    }

    pub fn inflight(&self) -> usize {
        *self.inflight.lock()
    }

    /// Look up a thumbnail, queueing a decode the first time it is asked for.
    pub fn get(&self, path: &Path, mtime: Option<SystemTime>, size: u32) -> State {
        let key = Key {
            path_hash: hash_path(path),
            mtime: mtime
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
            // Bucket sizes so dragging the zoom slider reuses decodes.
            bucket: bucket_for(size),
        };
        if let Some(state) = self.cache.lock().get(&key) {
            return state.clone();
        }
        {
            let mut cache = self.cache.lock();
            if cache.contains_key(&key) {
                return cache[&key].clone();
            }
            cache.insert(key, State::Pending);
            self.order.lock().push(key);
            *self.inflight.lock() += 1;
        }
        let _ = self.jobs.send(Job {
            key,
            path: path.to_path_buf(),
            size: key.bucket,
            generation: self.generation.load(Ordering::Relaxed),
        });
        State::Pending
    }

    pub fn clear(&self) {
        self.cache.lock().clear();
        self.order.lock().clear();
        self.new_generation();
    }

    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::fmt::Debug for Thumbnails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Thumbnails")
            .field("cached", &self.len())
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

/// Round a requested size up to the next power-of-two-ish bucket.
pub fn bucket_for(size: u32) -> u32 {
    const BUCKETS: [u32; 6] = [48, 64, 96, 128, 192, 256];
    *BUCKETS
        .iter()
        .find(|b| **b >= size)
        .unwrap_or(BUCKETS.last().unwrap())
}

fn hash_path(path: &Path) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

fn decode(path: &Path, size: u32) -> Option<Thumb> {
    let reader = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?;
    let source = reader.into_dimensions().ok()?;
    // Re-open: dimensions consumed the reader, and knowing them first lets us
    // skip absurdly large images that would blow up memory.
    if source.0 as u64 * source.1 as u64 > 64 * 1024 * 1024 {
        return None;
    }
    let img = image::ImageReader::open(path)
        .ok()?
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    let thumb = img.thumbnail(size, size).to_rgba8();
    Some(Thumb {
        width: thumb.width(),
        height: thumb.height(),
        rgba: Arc::new(thumb.into_raw()),
        source,
    })
}

/// Decode an image at up to `max` pixels on its long edge, for the preview
/// pane. Runs on the caller's thread; the UI calls it from a worker.
pub fn decode_preview(path: &Path, max: u32) -> Option<Thumb> {
    decode(path, max)
}

#[cfg(test)]
mod tests {
    #[test]
    fn buckets_round_up() {
        assert_eq!(super::bucket_for(40), 48);
        assert_eq!(super::bucket_for(100), 128);
        assert_eq!(super::bucket_for(5000), 256);
    }
}
