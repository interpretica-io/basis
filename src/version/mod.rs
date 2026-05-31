mod cpp;
mod rust;

use anyhow::{bail, Result};
use colored::Colorize;
use semver::Version;

use crate::config::{Config, Lang, Repo};

/// How to compute a component's new version in `bump`.
pub enum Bump {
    Major,
    Minor,
    Patch,
    To(String),
}

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

/// The package name a repo exposes to dependents: explicit `provides`, else the
/// Rust crate name, else the repo name.
fn provided_name(cfg: &Config, repo: &Repo) -> String {
    if let Some(p) = &repo.provides {
        return p.clone();
    }
    if repo.lang == Lang::Rust {
        if let Ok(Some(name)) = rust::read_package_name(&cfg.repo_dir(repo)) {
            return name;
        }
    }
    repo.name.clone()
}

/// Apply a `Bump` to a semver string.
fn apply_bump(current: &str, how: &Bump) -> Result<String> {
    if let Bump::To(v) = how {
        validate_semver(v)?;
        return Ok(v.clone());
    }
    let mut v = Version::parse(current).map_err(|_| {
        anyhow::anyhow!("current version '{current}' is not valid semver; use --to")
    })?;
    match how {
        Bump::Major => {
            v.major += 1;
            v.minor = 0;
            v.patch = 0;
        }
        Bump::Minor => {
            v.minor += 1;
            v.patch = 0;
        }
        Bump::Patch => v.patch += 1,
        Bump::To(_) => unreachable!(),
    }
    v.pre = semver::Prerelease::EMPTY;
    v.build = semver::BuildMetadata::EMPTY;
    Ok(v.to_string())
}

/// Bump one component's version, then update every repo that depends on it.
pub fn bump(cfg: &Config, repo_name: &str, how: Bump) -> Result<()> {
    let target = cfg
        .manifest
        .repos
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| anyhow::anyhow!("unknown repository '{repo_name}'"))?;

    let current = read_repo(cfg, target)?
        .ok_or_else(|| anyhow::anyhow!("'{repo_name}' has no readable version"))?;
    let new_version = apply_bump(&current, &how)?;

    let dep_name = provided_name(cfg, target);

    println!(
        "{} {} {} -> {} {}",
        "bumping".bold(),
        repo_name.cyan(),
        current.dimmed(),
        new_version.green(),
        format!("(provides '{dep_name}')").dimmed()
    );

    if new_version == current {
        println!("  {} already at {new_version}", "·".dimmed());
    } else {
        write_repo(cfg, target, &new_version)?;
        println!("  {} {} version set to {}", "✓".green(), repo_name, new_version);
    }

    // Propagate the new version into every other repo that depends on it.
    let mut touched = 0;
    for dependent in &cfg.manifest.repos {
        if dependent.name == repo_name {
            continue;
        }
        let dir = cfg.repo_dir(dependent);
        let changed = match dependent.lang {
            Lang::Rust => rust::update_dependency(&dir, &dep_name, &new_version)?,
            Lang::Cpp => cpp::update_dependency(&dir, dependent, &dep_name, &new_version)?,
        };
        if changed > 0 {
            touched += 1;
            println!(
                "  {} {} now requires {} {}",
                "↳".blue(),
                dependent.name,
                dep_name,
                new_version
            );
        }
    }

    if touched == 0 {
        println!("  {} no dependents referenced '{dep_name}'", "·".dimmed());
    }
    Ok(())
}

fn validate_semver(v: &str) -> Result<()> {
    if Version::parse(v).is_err() {
        bail!("'{v}' is not a valid semver version (expected MAJOR.MINOR.PATCH)");
    }
    Ok(())
}
