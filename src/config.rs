use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Language of a repository, which decides how versions are read and written.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Rust,
    Cpp,
}

impl std::fmt::Display for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lang::Rust => write!(f, "rust"),
            Lang::Cpp => write!(f, "cpp"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Repo {
    /// Human-readable, unique name used on the command line.
    pub name: String,
    /// Path to the repository, relative to the manifest.
    pub path: PathBuf,
    /// Canonical git URL the repository should originate from. Compared (after
    /// normalisation) against the local `origin` remote in `basis status`.
    #[serde(default)]
    pub url: Option<String>,
    /// Language, determines versioning strategy.
    pub lang: Lang,
    /// Package name this repo exposes to dependents (default: Rust crate name,
    /// otherwise the repo name). Used to find/patch cross-repo dependencies.
    #[serde(default)]
    pub provides: Option<String>,
    /// C++ only: file holding the plain version string (default `.version`).
    #[serde(default)]
    pub version_file: Option<PathBuf>,
    /// C++ only: CMake file whose `project(... VERSION ...)` is patched.
    #[serde(default)]
    pub cmake_file: Option<PathBuf>,
    /// Allowed e-mail domain(s) for this repo's git/GPG identity. Overrides the
    /// manifest-level policy when set. Single-value convenience field.
    #[serde(default)]
    pub email_domain: Option<String>,
    /// Allowed e-mail domains (list form). Merged with `email_domain`.
    #[serde(default)]
    pub email_domains: Vec<String>,
    /// Map of action name -> ordered list of shell commands.
    #[serde(default)]
    pub actions: BTreeMap<String, Vec<String>>,
}

/// A single pane within a tmux display.
#[derive(Debug, Deserialize)]
pub struct Pane {
    /// Optional pane title (defaults to the repo name, action, or "shell").
    #[serde(default)]
    pub name: Option<String>,
    /// Repository whose directory the pane starts in (and whose `action` runs).
    #[serde(default)]
    pub repo: Option<String>,
    /// Raw shell command to run in the pane.
    #[serde(default)]
    pub command: Option<String>,
    /// Name of a repo action to run in the pane (mutually used with `repo`).
    #[serde(default)]
    pub action: Option<String>,
    /// Explicit working directory (relative to the manifest), overrides `repo`.
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

/// A tmux "display": a session laid out with one pane per task.
#[derive(Debug, Deserialize)]
pub struct Display {
    /// tmux session name (default: `<constellation>-<display>`).
    #[serde(default)]
    pub session: Option<String>,
    /// tmux layout applied after panes are created.
    #[serde(default = "default_layout")]
    pub layout: String,
    pub panes: Vec<Pane>,
}

fn default_layout() -> String {
    "tiled".to_string()
}

/// Per-task execution options (keyed by action name).
#[derive(Debug, Deserialize, Default)]
pub struct TaskConfig {
    /// Name of the tmux display (session) to run this task in. When set, the
    /// task runs in a per-task tmux display (one pane per repo); otherwise it
    /// runs inline in the current terminal.
    #[serde(default)]
    pub display: Option<String>,
    /// tmux layout to use when the task runs in a display.
    #[serde(default)]
    pub layout: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub constellation: String,
    /// Optional canonical version of the whole constellation.
    #[serde(default)]
    pub version: Option<String>,
    /// Per-task execution options, keyed by action name.
    #[serde(default)]
    pub tasks: BTreeMap<String, TaskConfig>,
    /// Default allowed e-mail domain for git/GPG identity (single-value form).
    #[serde(default)]
    pub email_domain: Option<String>,
    /// Default allowed e-mail domains (list form). Merged with `email_domain`.
    #[serde(default)]
    pub email_domains: Vec<String>,
    pub repos: Vec<Repo>,
    /// Named tmux displays describing dev dashboards for the constellation.
    #[serde(default)]
    pub displays: BTreeMap<String, Display>,
}

/// A loaded manifest together with the directory it lives in.
#[derive(Debug)]
pub struct Config {
    pub manifest: Manifest,
    /// Directory of the manifest; all repo paths are resolved against it.
    pub base_dir: PathBuf,
}

impl Config {
    /// Resolve the manifest path. A bare filename (e.g. the default
    /// `basis.yaml`) is searched for upward from the current directory, like
    /// git looks for `.git`. A path with a directory component is used as-is.
    pub fn resolve_path(file: &Path) -> Result<PathBuf> {
        let has_dir = file
            .parent()
            .map(|p| !p.as_os_str().is_empty())
            .unwrap_or(false);
        if has_dir {
            return Ok(file.to_path_buf());
        }

        let cwd = std::env::current_dir().context("getting current directory")?;
        let mut dir = cwd.as_path();
        loop {
            let candidate = dir.join(file);
            if candidate.is_file() {
                return Ok(candidate);
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => break,
            }
        }
        bail!(
            "no {} found in {} or any parent directory",
            file.display(),
            cwd.display()
        );
    }

    pub fn load(file: &Path) -> Result<Self> {
        let path = Self::resolve_path(file)?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        let manifest: Manifest = serde_yaml::from_str(&text)
            .with_context(|| format!("parsing manifest {}", path.display()))?;

        let base_dir = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        // Reject duplicate repo names early — they would make `--repo` ambiguous.
        let mut seen = std::collections::HashSet::new();
        for repo in &manifest.repos {
            if !seen.insert(repo.name.as_str()) {
                bail!("duplicate repository name '{}'", repo.name);
            }
        }

        Ok(Config { manifest, base_dir })
    }

    /// Absolute (or manifest-relative) directory of a repository.
    pub fn repo_dir(&self, repo: &Repo) -> PathBuf {
        self.base_dir.join(&repo.path)
    }

    /// The configured display (tmux session) name for `action`, if it runs in
    /// a display.
    pub fn task_display(&self, action: &str) -> Option<String> {
        self.manifest
            .tasks
            .get(action)
            .and_then(|t| t.display.clone())
    }

    /// The configured tmux layout for `action`, if any.
    pub fn task_layout(&self, action: &str) -> Option<String> {
        self.manifest
            .tasks
            .get(action)
            .and_then(|t| t.layout.clone())
    }

    /// Look up a repository by name.
    pub fn find_repo(&self, name: &str) -> Result<&Repo> {
        self.manifest
            .repos
            .iter()
            .find(|r| r.name == name)
            .with_context(|| format!("unknown repository '{name}'"))
    }

    /// Allowed e-mail domains for a repo's identity: the repo-level policy if
    /// any, otherwise the manifest-level one. Lower-cased, deduplicated.
    pub fn allowed_domains(&self, repo: &Repo) -> Vec<String> {
        fn merge(single: &Option<String>, list: &[String]) -> Vec<String> {
            let mut v: Vec<String> = list.to_vec();
            if let Some(s) = single {
                v.push(s.clone());
            }
            v
        }

        let mut v = merge(&repo.email_domain, &repo.email_domains);
        if v.is_empty() {
            v = merge(&self.manifest.email_domain, &self.manifest.email_domains);
        }

        let mut out: Vec<String> = v
            .iter()
            .map(|d| d.trim().trim_start_matches('@').to_lowercase())
            .filter(|d| !d.is_empty())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Select repositories by name, preserving manifest order. An empty filter
    /// selects everything.
    pub fn select<'a>(&'a self, names: &[String]) -> Result<Vec<&'a Repo>> {
        if names.is_empty() {
            return Ok(self.manifest.repos.iter().collect());
        }
        let mut out = Vec::new();
        for name in names {
            let repo = self
                .manifest
                .repos
                .iter()
                .find(|r| &r.name == name)
                .with_context(|| format!("unknown repository '{name}'"))?;
            out.push(repo);
        }
        Ok(out)
    }
}
