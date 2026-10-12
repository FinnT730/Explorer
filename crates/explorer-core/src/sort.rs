use std::cmp::Ordering;

use crate::entry::Entry;

/// Column a listing is sorted by. Mirrors the Explorer "Sort by" menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum SortKey {
    Name,
    Size,
    Kind,
    Modified,
    Created,
    Extension,
}

impl SortKey {
    pub const ALL: [SortKey; 6] = [
        SortKey::Name,
        SortKey::Size,
        SortKey::Kind,
        SortKey::Modified,
        SortKey::Created,
        SortKey::Extension,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "Name",
            SortKey::Size => "Size",
            SortKey::Kind => "Kind",
            SortKey::Modified => "Date modified",
            SortKey::Created => "Date created",
            SortKey::Extension => "Type",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn flipped(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct SortSpec {
    pub key: SortKey,
    pub dir: SortDir,
    /// Keep directories above files regardless of the active column.
    pub dirs_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            dir: SortDir::Asc,
            dirs_first: true,
        }
    }
}

impl SortSpec {
    /// Clicking a column header: same column flips direction, new column
    /// starts ascending (descending for the time columns, like Explorer).
    pub fn toggle(&mut self, key: SortKey) {
        if self.key == key {
            self.dir = self.dir.flipped();
        } else {
            self.key = key;
            self.dir = match key {
                SortKey::Modified | SortKey::Created | SortKey::Size => SortDir::Desc,
                _ => SortDir::Asc,
            };
        }
    }

    pub fn sort(&self, entries: &mut [Entry]) {
        entries.sort_by(|a, b| self.cmp(a, b));
    }

    fn cmp(&self, a: &Entry, b: &Entry) -> Ordering {
        if self.dirs_first {
            match (a.is_dir, b.is_dir) {
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {}
            }
        }
        let ord = match self.key {
            SortKey::Name => natural_cmp(&a.name_lower, &b.name_lower),
            // Folders have no size, so they fall back to name within their group.
            SortKey::Size => a
                .size
                .unwrap_or(0)
                .cmp(&b.size.unwrap_or(0))
                .then_with(|| natural_cmp(&a.name_lower, &b.name_lower)),
            SortKey::Kind => a
                .class
                .cmp(&b.class)
                .then_with(|| natural_cmp(&a.name_lower, &b.name_lower)),
            SortKey::Modified => a
                .modified
                .cmp(&b.modified)
                .then_with(|| natural_cmp(&a.name_lower, &b.name_lower)),
            SortKey::Created => a
                .created
                .cmp(&b.created)
                .then_with(|| natural_cmp(&a.name_lower, &b.name_lower)),
            SortKey::Extension => a
                .ext
                .cmp(&b.ext)
                .then_with(|| natural_cmp(&a.name_lower, &b.name_lower)),
        };
        match self.dir {
            SortDir::Asc => ord,
            SortDir::Desc => ord.reverse(),
        }
    }
}

/// Compare two strings the way Explorer does: runs of digits compare as
/// numbers, so `file2` sorts before `file10`.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.char_indices().peekable();
    let mut bi = b.char_indices().peekable();

    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some((apos, ac)), Some((bpos, bc))) => {
                if ac.is_ascii_digit() && bc.is_ascii_digit() {
                    let an = take_digits(a, apos, &mut ai);
                    let bn = take_digits(b, bpos, &mut bi);
                    // Compare by value, then by width so `01` < `1`.
                    let ord = trim_zeros(an)
                        .len()
                        .cmp(&trim_zeros(bn).len())
                        .then_with(|| trim_zeros(an).cmp(trim_zeros(bn)))
                        .then_with(|| an.len().cmp(&bn.len()).reverse());
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    ai.next();
                    bi.next();
                    let ord = ac.cmp(&bc);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                }
            }
        }
    }
}

fn take_digits<'a, I>(src: &'a str, start: usize, it: &mut std::iter::Peekable<I>) -> &'a str
where
    I: Iterator<Item = (usize, char)>,
{
    let mut end = start;
    while let Some(&(pos, c)) = it.peek() {
        if c.is_ascii_digit() {
            end = pos + c.len_utf8();
            it.next();
        } else {
            break;
        }
    }
    &src[start..end]
}

fn trim_zeros(s: &str) -> &str {
    let t = s.trim_start_matches('0');
    if t.is_empty() {
        "0"
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_compare_numerically() {
        assert_eq!(natural_cmp("file2", "file10"), Ordering::Less);
        assert_eq!(natural_cmp("file10", "file2"), Ordering::Greater);
        assert_eq!(natural_cmp("a1b2", "a1b2"), Ordering::Equal);
    }

    #[test]
    fn plain_text_compares_lexically() {
        assert_eq!(natural_cmp("apple", "banana"), Ordering::Less);
        assert_eq!(natural_cmp("abc", "abcd"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_break_ties() {
        assert_eq!(natural_cmp("img01", "img1"), Ordering::Less);
    }
}
