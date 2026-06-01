use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use colored::Colorize;

use crate::config::Config;

/// Install a whole constellation: clone the manifest repository, read its
/// `basis.yaml`, then clone every member repository next to it.
///
/// `spec` is either an `org/repo` GitHub shorthand or a full git URL.
/// `manifest_file` is the manifest filename to look for inside the clone
/// (the global `--file`, default `basis.yaml`).
pub fn run(
    spec: &str,
    into: Option<PathBuf>,
    branch: Option<String>,
    manifest_file: &Path,
) -> Result<()> {
    let (url, default_dir) = resolve(spec);
    let target = into.unwrap_or_else(|| PathBuf::from(&default_dir));

    if is_non_empty_dir(&target) {
        bail!(
            "target directory {} already exists and is not empty",
            target.display()
        );
    }

    println!(
        "{} {} {} {}",
        "cloning manifest".bold(),
        url.cyan(),
        "->".dimmed(),
        target.display()
    );
    git_clone(&url, &target, branch.as_deref())?;

    // Locate the manifest inside the freshly cloned repo.
    let file_name = manifest_file
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("basis.yaml"));
    let manifest_path = target.join(&file_name);
    if !manifest_path.exists() {
        bail!(
            "{} not found in {} — is this a constellation manifest repo?",
            file_name.display(),
            url
        );
    }

    let cfg = Config::load(&manifest_path)?;
    println!(
        "\n{} {} ({} repos)",
        "constellation:".bold(),
        cfg.manifest.constellation,
        cfg.manifest.repos.len()
    );

    clone_members(&cfg)
}

/// Clone each member repository that has a `url` into its `path`.
fn clone_members(cfg: &Config) -> Result<()> {
    let mut cloned = 0usize;
    let mut skipped = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for repo in &cfg.manifest.repos {
        let dest = cfg.repo_dir(repo);

        let Some(url) = &repo.url else {
            println!("  {} {}: no url, skipped", "·".dimmed(), repo.name);
            skipped += 1;
            continue;
        };
        if is_non_empty_dir(&dest) {
            println!("  {} {}: already present, skipped", "·".dimmed(), repo.name);
            skipped += 1;
            continue;
        }

        println!(
            "\n{} {} {}",
            "==>".blue().bold(),
            repo.name.bold(),
            format!("({url})").dimmed()
        );
        match git_clone(url, &dest, None) {
            Ok(()) => cloned += 1,
            Err(e) => {
                println!("  {} {e}", "failed:".red());
                failed.push(repo.name.clone());
            }
        }
    }

    println!(
        "\n{} {cloned} cloned, {skipped} skipped, {} failed",
        "done:".bold(),
        failed.len()
    );
    if !failed.is_empty() {
        bail!("failed to clone: {}", failed.join(", "));
    }
    Ok(())
}

/// Run `git clone [--branch B] <url> <dest>`, streaming output to the terminal.
fn git_clone(url: &str, dest: &Path, branch: Option<&str>) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    if let Some(b) = branch {
        cmd.args(["--branch", b]);
    }
    cmd.arg(url).arg(dest);

    let status = cmd
        .status()
        .context("failed to run git (is it installed?)")?;
    if !status.success() {
        bail!("git clone {url} {status}");
    }
    Ok(())
}

/// Resolve a spec into a `(clone_url, default_directory_name)` pair.
fn resolve(spec: &str) -> (String, String) {
    let spec = spec.trim();
    let url = if spec.contains("://") || spec.starts_with("git@") {
        spec.to_string()
    } else {
        // `org/repo` shorthand defaults to GitHub over HTTPS.
        format!("https://github.com/{}", spec.trim_end_matches('/'))
    };
    (url, derive_name(spec))
}

/// Last path segment of a spec/URL, without a trailing `.git`.
fn derive_name(spec: &str) -> String {
    spec.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("constellation")
        .trim_end_matches(".git")
        .to_string()
}

/// True if `path` is a directory that contains at least one entry.
fn is_non_empty_dir(path: &Path) -> bool {
    path.read_dir()
        .map(|mut d| d.next().is_some())
        .unwrap_or(false)
}
