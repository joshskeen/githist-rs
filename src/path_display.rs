//! Helpers for showing worktree paths in the branch list.

/// Replace a home-directory prefix with `~`.
pub fn collapse_home(path: &str, home: &str) -> String {
    let home = home.trim_end_matches('/');
    if path == home {
        return "~".to_string();
    }
    let prefix = format!("{home}/");
    if let Some(rest) = path.strip_prefix(&prefix) {
        format!("~/{rest}")
    } else {
        path.to_string()
    }
}

/// Truncate a path to at most `max_chars`, keeping start and end with an ellipsis.
pub fn truncate_middle(path: &str, max_chars: usize) -> String {
    let chars: Vec<char> = path.chars().collect();
    if chars.len() <= max_chars {
        return path.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    // Prefer leaving more of the end (directory name).
    let ellipsis = '…';
    let keep = max_chars - 1;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars[..head].iter().collect();
    out.push(ellipsis);
    out.extend(chars[chars.len() - tail..].iter());
    out
}

/// Format a worktree path for list display: collapse `$HOME`, then truncate.
pub fn format_worktree_path(path: &str, max_chars: usize) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let collapsed = if home.is_empty() {
        path.to_string()
    } else {
        collapse_home(path, &home)
    };
    truncate_middle(&collapsed, max_chars)
}

#[cfg(test)]
mod tests {
    use super::{collapse_home, truncate_middle};

    #[test]
    fn collapse_home_replaces_prefix() {
        let home = "/Users/josh";
        assert_eq!(collapse_home("/Users/josh/code/wt", home), "~/code/wt");
    }

    #[test]
    fn collapse_home_leaves_unrelated_paths() {
        assert_eq!(collapse_home("/tmp/wt", "/Users/josh"), "/tmp/wt");
    }

    #[test]
    fn truncate_middle_keeps_short_paths() {
        assert_eq!(truncate_middle("~/a/b", 20), "~/a/b");
    }

    #[test]
    fn truncate_middle_ellipses_long_paths() {
        let path = "~/code/very/long/path/to/worktree";
        let truncated = truncate_middle(path, 18);
        assert!(truncated.chars().count() <= 18);
        assert!(truncated.contains('…'));
    }
}
