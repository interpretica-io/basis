use std::process::Command;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::config::Config;
use crate::git;

/// Update the constellation's cloned repositories with `git pull --ff-only`.
/// `--ff-only` never creates a merge commit: it fast-forwards or fails loudly
/// if a repo has diverged, so local work is never silently merged.
pub fn run(cfg: &Config, repos: &[String]) -> Result<()> {
    let selected = cfg.select(repos)?;
    let mut updated = 0usize;
    let mut skipped = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for repo in selected {
        let dir = cfg.repo_dir(repo);
        if !dir.is_dir() || !git::status(&dir).is_repo {
            println!("  {} {}: not cloned, skipped", "·".dimmed(), repo.name);
            skipped += 1;
            continue;
        }

        println!(
            "\n{} {} {}",
            "==>".blue().bold(),
            repo.name.bold(),
            format!("({})", dir.display()).dimmed()
        );
        let ok = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["pull", "--ff-only"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if ok {
            updated += 1;
        } else {
            println!("{} {}", "failed:".red(), repo.name);
            failed.push(repo.name.clone());
        }
    }

    println!(
        "\n{} {updated} updated, {skipped} skipped, {} failed",
        "done:".bold(),
        failed.len()
    );
    if !failed.is_empty() {
        bail!("failed to update: {}", failed.join(", "));
    }
    Ok(())
}
