use std::path::Path;

use anyhow::{Context, Result};
use toml_edit::{value, DocumentMut};

fn manifest_path(repo_dir: &Path) -> std::path::PathBuf {
    repo_dir.join("Cargo.toml")
}

fn load(repo_dir: &Path) -> Result<Option<DocumentMut>> {
    let path = manifest_path(repo_dir);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let doc: DocumentMut = text
        .parse()
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(doc))
}

/// Read `[package].version` from a crate's Cargo.toml.
pub fn read_version(repo_dir: &Path) -> Result<Option<String>> {
    let Some(doc) = load(repo_dir)? else {
        return Ok(None);
    };
    Ok(doc
        .get("package")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_string))
}

/// Write `[package].version`, preserving formatting and comments.
pub fn write_version(repo_dir: &Path, version: &str) -> Result<()> {
    let path = manifest_path(repo_dir);
    let mut doc = load(repo_dir)?
        .with_context(|| format!("no Cargo.toml in {}", repo_dir.display()))?;

    if doc.get("package").and_then(|p| p.get("version")).is_none() {
        anyhow::bail!(
            "{} has no [package].version to update",
            path.display()
        );
    }
    doc["package"]["version"] = value(version);

    std::fs::write(&path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
