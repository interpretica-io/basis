mod cpp;
mod rust;

use anyhow::{bail, Result};
use colored::Colorize;
use semver::Version;

use crate::config::{Config, Lang, Repo};

/// A repository's resolved version, plus where it came from.
pub struct VersionInfo {
    pub name: String,
    pub lang: Lang,
    pub version: Option<String>,
}

/// Read the current version of one repository.
pub fn read_repo(cfg: &Config, repo: &Repo) -> Result<Option<String>> {
    let dir = cfg.repo_dir(repo);
    match repo.lang {
        Lang::Rust => rust::read_version(&dir),
        Lang::Cpp => cpp::read_version(&dir, repo),
    }
}

/// Write a version into one repository.
pub fn write_repo(cfg: &Config, repo: &Repo, version: &str) -> Result<()> {
    let dir = cfg.repo_dir(repo);
    match repo.lang {
        Lang::Rust => rust::write_version(&dir, version),
        Lang::Cpp => cpp::write_version(&dir, repo, version),
    }
}

/// Collect versions for every repository in the manifest.
pub fn collect(cfg: &Config) -> Vec<VersionInfo> {
    cfg.manifest
        .repos
        .iter()
        .map(|repo| VersionInfo {
            name: repo.name.clone(),
            lang: repo.lang,
            version: read_repo(cfg, repo).unwrap_or(None),
        })
        .collect()
}

pub fn show(cfg: &Config) -> Result<()> {
    let infos = collect(cfg);
    let target = consensus(&infos);

    println!("{} {}", "constellation:".bold(), cfg.manifest.constellation);
    if let Some(v) = &cfg.manifest.version {
        println!("{} {}", "manifest version:".bold(), v);
    }
    println!();

    let width = infos.iter().map(|i| i.name.len()).max().unwrap_or(4).max(4);
    for info in &infos {
        let ver = info.version.as_deref().unwrap_or("—");
        let marker = match (&info.version, &target) {
            (Some(v), Some(t)) if v == t => "✓".green(),
            (Some(_), Some(_)) => "✗".red(),
            _ => "?".yellow(),
        };
        println!(
            "  {marker} {:<width$}  {:<4}  {}",
            info.name,
            info.lang.to_string(),
            ver,
            width = width
        );
    }

    println!();
    report_sync(&infos, target.as_deref());
    Ok(())
}

/// The version shared by all repos that have one, if they agree.
fn consensus(infos: &[VersionInfo]) -> Option<String> {
    let mut versions = infos.iter().filter_map(|i| i.version.clone());
    let first = versions.next()?;
    if versions.all(|v| v == first) {
        Some(first)
    } else {
        None
    }
}

/// Highest semver among the repos (used as default sync target).
fn highest(infos: &[VersionInfo]) -> Option<String> {
    infos
        .iter()
        .filter_map(|i| i.version.as_ref())
        .filter_map(|v| Version::parse(v).ok().map(|p| (p, v.clone())))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, raw)| raw)
}

fn report_sync(infos: &[VersionInfo], target: Option<&str>) {
    match target {
        Some(t) => println!("{} all repos at {}", "in sync:".green().bold(), t),
        None => {
            let out: Vec<_> = infos
                .iter()
                .filter(|i| i.version.is_some())
                .map(|i| format!("{}={}", i.name, i.version.as_deref().unwrap()))
                .collect();
            println!("{} {}", "out of sync:".red().bold(), out.join(", "));
        }
    }
}

pub fn set_all(cfg: &Config, version: &str) -> Result<()> {
    validate_semver(version)?;
    apply(cfg, version)
}

pub fn sync(cfg: &Config, to: Option<&str>) -> Result<()> {
    let infos = collect(cfg);

    let target = match to {
        Some(v) => v.to_string(),
        None => cfg
            .manifest
            .version
            .clone()
            .or_else(|| highest(&infos))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "no sync target: pass --to, set `version:` in the manifest, \
                     or ensure at least one repo has a parseable version"
                )
            })?,
    };
    validate_semver(&target)?;

    println!("{} {}", "syncing to".bold(), target.cyan());
    apply(cfg, &target)
}

fn apply(cfg: &Config, version: &str) -> Result<()> {
    for repo in &cfg.manifest.repos {
        let before = read_repo(cfg, repo).unwrap_or(None);
        if before.as_deref() == Some(version) {
            println!("  {} {} already {}", "·".dimmed(), repo.name, version);
            continue;
        }
        write_repo(cfg, repo, version)?;
        let from = before.as_deref().unwrap_or("—");
        println!(
            "  {} {} {} -> {}",
            "✓".green(),
            repo.name,
            from.dimmed(),
            version
        );
    }
    Ok(())
}

fn validate_semver(v: &str) -> Result<()> {
    if Version::parse(v).is_err() {
        bail!("'{v}' is not a valid semver version (expected MAJOR.MINOR.PATCH)");
    }
    Ok(())
}
