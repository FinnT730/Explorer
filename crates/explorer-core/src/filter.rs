use crate::entry::{Entry, FileClass};

/// View filter applied after a scan: hidden-file visibility plus the search box.
#[derive(Clone, Debug, Default)]
pub struct Filter {
    pub show_hidden: bool,
    pub query: String,
    /// When set, only entries of this class are listed.
    pub class: Option<FileClass>,
}

impl Filter {
    pub fn matches(&self, e: &Entry) -> bool {
        if e.hidden && !self.show_hidden {
            return false;
        }
        if let Some(class) = self.class {
            if e.class != class {
                return false;
            }
        }
        if self.query.is_empty() {
            return true;
        }
        matches_query(&e.name_lower, &self.query.to_lowercase())
    }

    pub fn is_searching(&self) -> bool {
        !self.query.trim().is_empty()
    }
}

/// Substring match, with `*` and `?` wildcards so `*.rs` works like it does in
/// the Explorer search box.
pub fn matches_query(name: &str, query: &str) -> bool {
    if query.contains('*') || query.contains('?') {
        glob_match(name.as_bytes(), query.as_bytes())
    } else {
        name.contains(query)
    }
}

fn glob_match(name: &[u8], pat: &[u8]) -> bool {
    // Iterative backtracking matcher: no allocation, linear in the common case.
    let (mut n, mut p) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while n < name.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == name[n]) {
            n += 1;
            p += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = p;
            mark = n;
            p += 1;
        } else if star != usize::MAX {
            p = star + 1;
            mark += 1;
            n = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        assert!(matches_query("main.rs", "*.rs"));
        assert!(!matches_query("main.rso", "*.rs"));
        assert!(matches_query("a1.txt", "a?.txt"));
        assert!(matches_query("report-2024.pdf", "report*"));
    }

    #[test]
    fn plain_substring() {
        assert!(matches_query("my report.pdf", "report"));
        assert!(!matches_query("my report.pdf", "zzz"));
    }
}
