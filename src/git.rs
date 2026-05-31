use std::path::Path;
use std::process::Command;

/// Summary of a repository's working tree, as reported by git.
#[derive(Debug, Default)]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: String,
    pub dirty: bool,
    pub ahead: usize,
    pub behind: usize,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Read an effective git config value for `dir` (local, then global, etc.).
/// Returns `None` if unset or empty.
pub fn config(dir: &Path, key: &str) -> Option<String> {
    git(dir, &["config", "--get", key]).filter(|s| !s.is_empty())
}

/// Collect git status for `dir`. Never fails: non-repos return `is_repo: false`.
pub fn status(dir: &Path) -> GitStatus {
    let mut st = GitStatus::default();

    if git(dir, &["rev-parse", "--is-inside-work-tree"]).as_deref() != Some("true") {
        return st;
    }
    st.is_repo = true;

    st.branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();

    if let Some(porcelain) = git(dir, &["status", "--porcelain"]) {
        st.dirty = !porcelain.is_empty();
    }

    // ahead/behind vs upstream; absent upstream simply leaves these at 0.
    if let Some(counts) = git(
        dir,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) {
        let mut it = counts.split_whitespace();
        st.behind = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        st.ahead = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    }

    st
}
