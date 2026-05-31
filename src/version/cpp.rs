use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;

use crate::config::Repo;

/// Path to the plain-text version file (default `.version`).
pub fn version_file(repo_dir: &Path, repo: &Repo) -> PathBuf {
    let rel = repo
        .version_file
        .clone()
        .unwrap_or_else(|| PathBuf::from(".version"));
    repo_dir.join(rel)
}

/// Path to the CMake file to patch (default `CMakeLists.txt`).
fn cmake_file(repo_dir: &Path, repo: &Repo) -> PathBuf {
    let rel = repo
        .cmake_file
        .clone()
        .unwrap_or_else(|| PathBuf::from("CMakeLists.txt"));
    repo_dir.join(rel)
}

/// Matches the version inside `project(<name> ... VERSION x.y.z ...)`.
/// Group 1 is everything up to and including `VERSION `, group 2 is the number.
fn cmake_version_re() -> Regex {
    Regex::new(r"(?is)(project\s*\(.*?\bVERSION\s+)(\d+(?:\.\d+){0,3})").unwrap()
}

/// Read the version for a C++ repo: prefer `.version`, fall back to CMake.
pub fn read_version(repo_dir: &Path, repo: &Repo) -> Result<Option<String>> {
    let vf = version_file(repo_dir, repo);
    if vf.exists() {
        let text = std::fs::read_to_string(&vf)
            .with_context(|| format!("reading {}", vf.display()))?;
        let v = text.trim().to_string();
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }

    let cf = cmake_file(repo_dir, repo);
    if cf.exists() {
        let text = std::fs::read_to_string(&cf)
            .with_context(|| format!("reading {}", cf.display()))?;
        if let Some(caps) = cmake_version_re().captures(&text) {
            return Ok(Some(caps[2].to_string()));
        }
    }

    Ok(None)
}

/// Write the version: always update `.version`, and patch CMake if present.
pub fn write_version(repo_dir: &Path, repo: &Repo, version: &str) -> Result<()> {
    let vf = version_file(repo_dir, repo);
    std::fs::write(&vf, format!("{version}\n"))
        .with_context(|| format!("writing {}", vf.display()))?;

    let cf = cmake_file(repo_dir, repo);
    if cf.exists() {
        let text = std::fs::read_to_string(&cf)
            .with_context(|| format!("reading {}", cf.display()))?;
        let re = cmake_version_re();
        if re.is_match(&text) {
            let patched = re.replace(&text, |caps: &regex::Captures| {
                format!("{}{}", &caps[1], version)
            });
            std::fs::write(&cf, patched.as_ref())
                .with_context(|| format!("writing {}", cf.display()))?;
        }
    }

    Ok(())
}
