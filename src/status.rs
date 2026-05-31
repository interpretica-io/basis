use anyhow::Result;
use colored::Colorize;

use crate::config::Config;
use crate::git;
use crate::version;

/// Show git + version status for the whole constellation.
pub fn show(cfg: &Config) -> Result<()> {
    println!("{} {}", "constellation:".bold(), cfg.manifest.constellation);
    if let Some(v) = &cfg.manifest.version {
        println!("{} {}", "manifest version:".bold(), v);
    }
    println!();

    let infos = version::collect(cfg);
    let name_w = cfg
        .manifest
        .repos
        .iter()
        .map(|r| r.name.len())
        .max()
        .unwrap_or(4)
        .max(4);

    for (repo, info) in cfg.manifest.repos.iter().zip(&infos) {
        let dir = cfg.repo_dir(repo);
        let g = git::status(&dir);

        let git_col = if !g.is_repo {
            "not a git repo".dimmed().to_string()
        } else {
            let mut parts = vec![g.branch.cyan().to_string()];
            parts.push(if g.dirty {
                "dirty".red().to_string()
            } else {
                "clean".green().to_string()
            });
            if g.ahead > 0 {
                parts.push(format!("↑{}", g.ahead).yellow().to_string());
            }
            if g.behind > 0 {
                parts.push(format!("↓{}", g.behind).yellow().to_string());
            }
            parts.join(" ")
        };

        let ver = info.version.as_deref().unwrap_or("—");

        println!(
            "  {:<name_w$}  {:<4}  {:<10}  {}",
            repo.name.bold(),
            info.lang.to_string(),
            ver,
            git_col,
            name_w = name_w
        );
    }

    println!();
    let all: Vec<_> = infos.iter().filter_map(|i| i.version.clone()).collect();
    let synced = !all.is_empty() && all.iter().all(|v| *v == all[0]);
    if synced {
        println!("{} all versions at {}", "versions:".green().bold(), all[0]);
    } else {
        println!(
            "{} versions differ across repos (run `basis version sync`)",
            "versions:".red().bold()
        );
    }

    Ok(())
}
