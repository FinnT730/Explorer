//! Core filesystem model for Explorer.
//!
//! Everything in this crate is UI-agnostic: it scans directories, sorts and
//! filters entries, tracks navigation history, enumerates volumes and performs
//! file operations. The UI crate is a thin layer on top of it.

pub mod entry;
pub mod filter;
pub mod fmt;
pub mod nav;
pub mod ops;
pub mod places;
pub mod scan;
pub mod sort;
pub mod watch;

pub use entry::{Entry, FileClass};
pub use filter::Filter;
pub use nav::History;
pub use places::{Place, PlaceKind, Places, Volume};
pub use scan::{ScanResult, Scanner};
pub use sort::{natural_cmp, SortDir, SortKey, SortSpec};
pub use watch::DirWatcher;
