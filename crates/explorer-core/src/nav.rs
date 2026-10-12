use std::path::{Path, PathBuf};

/// Back/forward stacks for one tab, with the same semantics as the Explorer
/// navigation buttons: going somewhere new clears the forward stack.
#[derive(Clone, Debug, Default)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    current: PathBuf,
    limit: usize,
}

impl History {
    pub fn new(start: PathBuf) -> Self {
        Self {
            back: Vec::new(),
            forward: Vec::new(),
            current: start,
            limit: 256,
        }
    }

    pub fn current(&self) -> &Path {
        &self.current
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn can_go_up(&self) -> bool {
        self.current.parent().is_some()
    }

    /// Navigate to `path`, pushing the old location onto the back stack.
    /// Re-navigating to the same directory is a no-op (use it for refresh).
    pub fn push(&mut self, path: PathBuf) -> bool {
        if path == self.current {
            return false;
        }
        self.back.push(std::mem::replace(&mut self.current, path));
        if self.back.len() > self.limit {
            self.back.remove(0);
        }
        self.forward.clear();
        true
    }

    pub fn back(&mut self) -> Option<&Path> {
        let prev = self.back.pop()?;
        self.forward
            .push(std::mem::replace(&mut self.current, prev));
        Some(&self.current)
    }

    pub fn forward(&mut self) -> Option<&Path> {
        let next = self.forward.pop()?;
        self.back.push(std::mem::replace(&mut self.current, next));
        Some(&self.current)
    }

    pub fn up(&mut self) -> Option<&Path> {
        let parent = self.current.parent()?.to_path_buf();
        self.push(parent);
        Some(&self.current)
    }

    /// Recent locations for the back-button drop-down, newest first.
    pub fn recent_back(&self, n: usize) -> impl Iterator<Item = &Path> {
        self.back.iter().rev().take(n).map(|p| p.as_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_forward_round_trip() {
        let mut h = History::new(PathBuf::from("/a"));
        assert!(!h.can_go_back());
        h.push(PathBuf::from("/b"));
        h.push(PathBuf::from("/c"));
        assert_eq!(h.back().unwrap(), Path::new("/b"));
        assert_eq!(h.forward().unwrap(), Path::new("/c"));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn new_navigation_clears_forward() {
        let mut h = History::new(PathBuf::from("/a"));
        h.push(PathBuf::from("/b"));
        h.back();
        h.push(PathBuf::from("/z"));
        assert!(!h.can_go_forward());
    }
}
