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
    /// Language, determines versioning strategy.
    pub lang: Lang,
    /// C++ only: file holding the plain version string (default `.version`).
    #[serde(default)]
    pub version_file: Option<PathBuf>,
    /// C++ only: CMake file whose `project(... VERSION ...)` is patched.
    #[serde(default)]
    pub cmake_file: Option<PathBuf>,
    /// Map of action name -> ordered list of shell commands.
    #[serde(default)]
    pub actions: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub constellation: String,
    /// Optional canonical version of the whole constellation.
    #[serde(default)]
    pub version: Option<String>,
    pub repos: Vec<Repo>,
}

/// A loaded manifest together with the directory it lives in.
#[derive(Debug)]
pub struct Config {
    pub manifest: Manifest,
    /// Directory of the manifest; all repo paths are resolved against it.
    pub base_dir: PathBuf,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
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
