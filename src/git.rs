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

/// URL of a named remote (e.g. "origin"), or `None` if it has none.
pub fn remote_url(dir: &Path, remote: &str) -> Option<String> {
    git(dir, &["remote", "get-url", remote]).filter(|s| !s.is_empty())
}

/// Normalize a git URL so equivalent spellings compare equal, e.g.
/// `git@github.com:org/repo.git` and `https://github.com/org/repo` both
/// become `github.com/org/repo`.
pub fn normalize_url(url: &str) -> String {
    let mut s = url.trim().to_string();

    // Drop the scheme (https://, ssh://, git://, ...).
    if let Some(idx) = s.find("://") {
        s = s[idx + 3..].to_string();
    }

    // Drop userinfo (git@host) in the authority section.
    let authority_end = s.find('/').unwrap_or(s.len());
    if let Some(at) = s[..authority_end].find('@') {
        s = s[at + 1..].to_string();
    }

    // scp-like `host:path` -> `host/path` (turn the authority ':' into '/').
    let authority_end = s.find('/').unwrap_or(s.len());
    if let Some(colon) = s[..authority_end].find(':') {
        s.replace_range(colon..colon + 1, "/");
    }

    // Drop trailing `.git` and slashes.
    let trimmed = s.trim_end_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    trimmed.trim_end_matches('/').to_lowercase()
}

/// Whether two git URLs refer to the same repository.
pub fn same_remote(a: &str, b: &str) -> bool {
    normalize_url(a) == normalize_url(b)
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
