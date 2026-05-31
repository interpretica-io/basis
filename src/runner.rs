use std::process::Command;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::cli::RunArgs;
use crate::config::Config;

/// Run a named action (e.g. "build" or "clean") across the selected repos.
pub fn run_action(cfg: &Config, action: &str, args: &RunArgs) -> Result<()> {
    let repos = cfg.select(&args.repos)?;
    let mut failures: Vec<String> = Vec::new();

    for repo in repos {
        let Some(commands) = repo.actions.get(action) else {
            println!(
                "{} {} has no '{}' action — skipping",
                "·".dimmed(),
                repo.name.yellow(),
                action
            );
            continue;
        };

        let dir = cfg.repo_dir(repo);
        println!(
            "\n{} {} {}",
            "==>".blue().bold(),
            repo.name.bold(),
            format!("({})", dir.display()).dimmed()
        );

        if !dir.is_dir() {
            let msg = format!("directory {} does not exist", dir.display());
            if args.keep_going || args.dry_run {
                println!("{} {msg}", "warning:".yellow());
                failures.push(repo.name.clone());
                continue;
            }
            bail!("{} in repo '{}'", msg, repo.name);
        }

        for cmd in commands {
            println!("{} {}", "$".green(), cmd);
            if args.dry_run {
                continue;
            }

            let status = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&dir)
                .status();

            let ok = matches!(&status, Ok(s) if s.success());
            if !ok {
                let detail = match status {
                    Ok(s) => format!("exited with {s}"),
                    Err(e) => format!("failed to spawn: {e}"),
                };
                if args.keep_going {
                    println!("{} {} ({detail})", "failed:".red(), repo.name);
                    failures.push(repo.name.clone());
                    break;
                }
                bail!("command in '{}' {detail}: {cmd}", repo.name);
            }
        }
    }

    if !failures.is_empty() {
        bail!("{} repo(s) failed: {}", failures.len(), failures.join(", "));
    }
    Ok(())
}
