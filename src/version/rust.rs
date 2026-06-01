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
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let doc: DocumentMut = text
        .parse()
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(doc))
}

/// Read `[package].name` (the crate name other repos depend on).
pub fn read_package_name(repo_dir: &Path) -> Result<Option<String>> {
    let Some(doc) = load(repo_dir)? else {
        return Ok(None);
    };
    Ok(doc
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|v| v.as_str())
        .map(str::to_string))
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
    let mut doc =
        load(repo_dir)?.with_context(|| format!("no Cargo.toml in {}", repo_dir.display()))?;

    if doc.get("package").and_then(|p| p.get("version")).is_none() {
        anyhow::bail!("{} has no [package].version to update", path.display());
    }
    doc["package"]["version"] = value(version);

    std::fs::write(&path, doc.to_string())
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Update every dependency on `dep_name` in this crate's Cargo.toml to
/// require `new_version`, preserving `path`, features and renames. Returns the
/// number of dependency entries changed.
pub fn update_dependency(repo_dir: &Path, dep_name: &str, new_version: &str) -> Result<usize> {
    let path = manifest_path(repo_dir);
    let Some(mut doc) = load(repo_dir)? else {
        return Ok(0);
    };

    let tables = ["dependencies", "dev-dependencies", "build-dependencies"];
    let mut changed = 0;

    for table_name in tables {
        let Some(table) = doc.get_mut(table_name).and_then(|t| t.as_table_like_mut()) else {
            continue;
        };

        // Collect matching keys first to avoid borrowing while mutating.
        let keys: Vec<String> = table
            .iter()
            .filter(|(key, item)| {
                // A `package = "..."` rename takes precedence over the key.
                let real = item.get("package").and_then(|v| v.as_str()).unwrap_or(key);
                real == dep_name
            })
            .map(|(key, _)| key.to_string())
            .collect();

        for key in keys {
            let Some(item) = table.get_mut(&key) else {
                continue;
            };
            if let Some(s) = item.as_str() {
                // Shorthand `dep = "1.0.0"`.
                if s != new_version {
                    *item = value(new_version);
                    changed += 1;
                }
            } else if let Some(t) = item.as_table_like_mut() {
                // Table form `dep = { version = "1.0.0", path = "..." }`.
                let cur = t.get("version").and_then(|v| v.as_str());
                if cur != Some(new_version) {
                    t.insert("version", value(new_version));
                    changed += 1;
                }
            }
        }
    }

    if changed > 0 {
        std::fs::write(&path, doc.to_string())
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(changed)
}
