use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{bail, Result};
use colored::Colorize;

use crate::cli::RunArgs;
use crate::config::Config;

/// List every action defined in the manifest, which repos provide it, and the
/// display it runs in (if any). Shown when `basis` is run with no subcommand.
pub fn list_actions(cfg: &Config) -> Result<()> {
    println!("{} {}", "constellation:".bold(), cfg.manifest.constellation);

    let mut actions: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for repo in &cfg.manifest.repos {
        for action in repo.actions.keys() {
            // Hide hooks like `_postclone` — they run automatically, not by name.
            if action.starts_with('_') {
                continue;
            }
            actions.entry(action).or_default().push(&repo.name);
        }
    }

    if actions.is_empty() {
        println!("\nno actions defined in the manifest");
        return Ok(());
    }

    println!("\n{}", "actions:".bold());
    let width = actions.keys().map(|a| a.len()).max().unwrap_or(0).max(4);
    for (action, repos) in &actions {
        let repolist = repos.join(", ");
        match cfg.task_display(action) {
            Some(name) => println!(
                "  {:<width$}  {}  {}",
                action,
                repolist.dimmed(),
                format!("→ display {name}").cyan(),
                width = width
            ),
            None => println!("  {:<width$}  {}", action, repolist.dimmed(), width = width),
        }
    }
    println!("\nrun any with {}", "basis <action>".bold());
    Ok(())
}

/// Run a named action (e.g. "build" or "clean") across the selected repos.
pub fn run_action(cfg: &Config, action: &str, args: &RunArgs) -> Result<()> {
    // A per-task tmux display runs the action in parallel, one pane per repo.
    // Enabled by the task's `display:` name (or forced with --tmux / --no-tmux).
    // The session is created lazily, only on execution.
    if let Some(session) = crate::display::resolve_session(cfg, action, args) {
        return crate::display::launch_task(cfg, action, args, &session);
    }

    let repos = cfg.select(&args.repos)?;

    // Catch typos / unknown actions: error if no selected repo defines it.
    if !repos.iter().any(|r| r.actions.contains_key(action)) {
        bail!("no selected repository defines a '{action}' action");
    }

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
