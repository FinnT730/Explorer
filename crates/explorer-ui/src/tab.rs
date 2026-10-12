//! Per-tab state: where it is, what it listed, what is selected.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use explorer_core::{entry::file_name_of, Entry, Filter, History, SortSpec};

use crate::settings::{Settings, ViewMode};

pub struct Tab {
    pub history: History,
    /// Everything the scan returned, unfiltered.
    pub entries: Vec<Entry>,
    /// Indices into `entries`, after filtering and sorting. This is what the
    /// views iterate, so filtering never copies entry data.
    pub view: Vec<usize>,
    pub filter: Filter,
    pub sort: SortSpec,
    pub view_mode: ViewMode,
    pub icon_size: f32,
    pub selection: HashSet<PathBuf>,
    /// Row the keyboard is on, as an index into `view`.
    pub cursor: Option<usize>,
    /// Shift-click range anchor, as an index into `view`.
    pub anchor: Option<usize>,
    /// Id of the listing scan we are waiting for, if any.
    pub pending_scan: Option<u64>,
    /// Id of the recursive search in flight, if any.
    pub pending_search: Option<u64>,
    pub error: Option<String>,
    /// Set when the filter or sort changed and `view` must be rebuilt.
    pub needs_rebuild: bool,
    /// Scroll to the cursor on the next frame (after keyboard navigation).
    pub scroll_to_cursor: bool,
    /// Name to rename inline, if a rename is in progress.
    pub renaming: Option<(PathBuf, String)>,
}

impl Tab {
    pub fn new(path: PathBuf, settings: &Settings) -> Self {
        Self {
            history: History::new(path),
            entries: Vec::new(),
            view: Vec::new(),
            filter: Filter {
                show_hidden: settings.show_hidden,
                query: String::new(),
                class: None,
            },
            sort: settings.sort,
            view_mode: settings.view_mode,
            icon_size: settings.icon_size,
            selection: HashSet::new(),
            cursor: None,
            anchor: None,
            pending_scan: None,
            pending_search: None,
            error: None,
            needs_rebuild: true,
            scroll_to_cursor: false,
            renaming: None,
        }
    }

    pub fn path(&self) -> &Path {
        self.history.current()
    }

    /// Tab strip label: the folder name, or the search text while searching.
    pub fn title(&self) -> String {
        if self.filter.is_searching() {
            format!("Search: {}", self.filter.query)
        } else {
            let name = file_name_of(self.path());
            if name.is_empty() {
                self.path().display().to_string()
            } else {
                name
            }
        }
    }

    pub fn is_loading(&self) -> bool {
        self.pending_scan.is_some() || self.pending_search.is_some()
    }

    /// Re-apply the filter and sort. Cheap: it sorts indices, not entries.
    pub fn rebuild(&mut self) {
        // Sort the backing vec once, then keep indices in that order; this
        // makes repeated filter edits (typing in the search box) allocation-free.
        self.sort.sort(&mut self.entries);
        self.view = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| self.filter.matches(e))
            .map(|(i, _)| i)
            .collect();
        self.needs_rebuild = false;
        // Drop selections that no longer exist in the listing.
        let live: HashSet<&PathBuf> = self.entries.iter().map(|e| &e.path).collect();
        self.selection.retain(|p| live.contains(p));
        if let Some(c) = self.cursor {
            if c >= self.view.len() {
                self.cursor = if self.view.is_empty() {
                    None
                } else {
                    Some(self.view.len() - 1)
                };
            }
        }
    }

    pub fn entry_at(&self, view_index: usize) -> Option<&Entry> {
        self.view.get(view_index).and_then(|i| self.entries.get(*i))
    }

    pub fn visible_entries(&self) -> impl Iterator<Item = &Entry> {
        self.view.iter().filter_map(|i| self.entries.get(*i))
    }

    pub fn selected_paths(&self) -> Vec<PathBuf> {
        // Return them in view order so copy/paste keeps the on-screen order.
        self.visible_entries()
            .filter(|e| self.selection.contains(&e.path))
            .map(|e| e.path.clone())
            .collect()
    }

    pub fn selected_entries(&self) -> impl Iterator<Item = &Entry> {
        self.visible_entries()
            .filter(|e| self.selection.contains(&e.path))
    }

    pub fn selection_size(&self) -> u64 {
        self.selected_entries().filter_map(|e| e.size).sum()
    }

    pub fn total_size(&self) -> u64 {
        self.visible_entries().filter_map(|e| e.size).sum()
    }

    pub fn select_only(&mut self, index: usize) {
        self.selection.clear();
        if let Some(e) = self.entry_at(index) {
            let path = e.path.clone();
            self.selection.insert(path);
        }
        self.cursor = Some(index);
        self.anchor = Some(index);
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(e) = self.entry_at(index) {
            let path = e.path.clone();
            if !self.selection.remove(&path) {
                self.selection.insert(path);
            }
        }
        self.cursor = Some(index);
        self.anchor = Some(index);
    }

    /// Shift-click / shift-arrow: select everything between anchor and index.
    pub fn select_range(&mut self, index: usize) {
        let anchor = self.anchor.unwrap_or(index);
        let (lo, hi) = if anchor <= index {
            (anchor, index)
        } else {
            (index, anchor)
        };
        self.selection.clear();
        for i in lo..=hi.min(self.view.len().saturating_sub(1)) {
            if let Some(e) = self.entry_at(i) {
                let p = e.path.clone();
                self.selection.insert(p);
            }
        }
        self.cursor = Some(index);
    }

    pub fn select_all(&mut self) {
        self.selection = self.visible_entries().map(|e| e.path.clone()).collect();
    }

    pub fn invert_selection(&mut self) {
        let next: std::collections::HashSet<PathBuf> = self
            .visible_entries()
            .filter(|e| !self.selection.contains(&e.path))
            .map(|e| e.path.clone())
            .collect();
        self.selection = next;
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.cursor = None;
        self.anchor = None;
    }

    pub fn move_cursor(&mut self, delta: i64, extend: bool) {
        if self.view.is_empty() {
            return;
        }
        let last = self.view.len() as i64 - 1;
        let next = match self.cursor {
            Some(c) => (c as i64 + delta).clamp(0, last),
            None if delta >= 0 => 0,
            None => last,
        } as usize;
        if extend {
            if self.anchor.is_none() {
                self.anchor = self.cursor;
            }
            self.select_range(next);
        } else {
            self.select_only(next);
        }
        self.scroll_to_cursor = true;
    }

    /// Jump to the first entry starting with `c`, like type-ahead in Explorer.
    pub fn type_ahead(&mut self, prefix: &str) {
        let needle = prefix.to_lowercase();
        let start = self.cursor.map(|c| c + 1).unwrap_or(0);
        let len = self.view.len();
        for off in 0..len {
            let i = (start + off) % len;
            if self
                .entry_at(i)
                .map(|e| e.name_lower.starts_with(&needle))
                .unwrap_or(false)
            {
                self.select_only(i);
                self.scroll_to_cursor = true;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use explorer_core::entry::Entry;
    use std::time::SystemTime;

    fn entry(name: &str, is_dir: bool, size: u64, hidden: bool) -> Entry {
        let path = PathBuf::from("/x").join(name);
        Entry {
            name_lower: name.to_lowercase(),
            name: name.to_string(),
            ext: if is_dir {
                String::new()
            } else {
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string()
            },
            class: if is_dir {
                explorer_core::FileClass::Folder
            } else {
                explorer_core::FileClass::Other
            },
            size: if is_dir { None } else { Some(size) },
            modified: Some(SystemTime::UNIX_EPOCH),
            created: None,
            accessed: None,
            hidden,
            readonly: false,
            is_dir,
            is_symlink: false,
            path,
        }
    }

    fn tab_with(entries: Vec<Entry>) -> Tab {
        let mut tab = Tab::new(PathBuf::from("/x"), &Settings::default());
        tab.entries = entries;
        tab.rebuild();
        tab
    }

    #[test]
    fn hidden_entries_are_filtered_until_asked_for() {
        let mut tab = tab_with(vec![
            entry("visible.txt", false, 1, false),
            entry(".secret", false, 1, true),
        ]);
        assert_eq!(tab.view.len(), 1);
        tab.filter.show_hidden = true;
        tab.rebuild();
        assert_eq!(tab.view.len(), 2);
    }

    #[test]
    fn folders_sort_before_files() {
        let tab = tab_with(vec![
            entry("a.txt", false, 1, false),
            entry("zzz", true, 0, false),
        ]);
        assert_eq!(tab.entry_at(0).unwrap().name, "zzz");
    }

    #[test]
    fn range_selection_covers_both_ends() {
        let mut tab = tab_with(vec![
            entry("a", false, 1, false),
            entry("b", false, 1, false),
            entry("c", false, 1, false),
        ]);
        tab.select_only(0);
        tab.select_range(2);
        assert_eq!(tab.selection.len(), 3);
        assert_eq!(tab.selection_size(), 3);
    }

    #[test]
    fn selection_survives_rescan_of_the_same_names() {
        let mut tab = tab_with(vec![
            entry("a", false, 1, false),
            entry("b", false, 1, false),
        ]);
        tab.select_only(1);
        tab.entries = vec![
            entry("a", false, 1, false),
            entry("b", false, 2, false),
            entry("c", false, 1, false),
        ];
        tab.rebuild();
        assert_eq!(tab.selected_paths(), vec![PathBuf::from("/x/b")]);
    }

    #[test]
    fn type_ahead_jumps_to_the_matching_row() {
        let mut tab = tab_with(vec![
            entry("alpha", false, 1, false),
            entry("beta", false, 1, false),
            entry("gamma", false, 1, false),
        ]);
        tab.type_ahead("g");
        assert_eq!(tab.cursor, Some(2));
    }
}
