use anyhow::Result;
use colored::Colorize;

use crate::config::Config;
use crate::display;
use crate::git;
use crate::verify::{self, State};
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

    let mut id_pass = 0usize;
    let mut id_fail = 0usize;
    let mut url_ok = 0usize;
    let mut url_bad = 0usize;
    let mut remote_notes: Vec<String> = Vec::new();

    for (repo, info) in cfg.manifest.repos.iter().zip(&infos) {
        let dir = cfg.repo_dir(repo);
        let g = git::status(&dir);

        // Compare the canonical URL (if any) with the local `origin` remote.
        let mut remote_token: Option<String> = None;
        if let Some(canon) = &repo.url {
            if !dir.is_dir() {
                url_bad += 1;
                remote_token = Some("missing".red().to_string());
                remote_notes.push(format!(
                    "  {} {}: not cloned — expected {}",
                    "✗".red(),
                    repo.name,
                    canon
                ));
            } else {
                match git::remote_url(&dir, "origin") {
                    Some(actual) if git::same_remote(&actual, canon) => {
                        url_ok += 1;
                        remote_token = Some("origin✓".green().to_string());
                    }
                    Some(actual) => {
                        url_bad += 1;
                        remote_token = Some("origin✗".red().to_string());
                        remote_notes.push(format!(
                            "  {} {}: origin is {} — expected {}",
                            "✗".red(),
                            repo.name,
                            actual,
                            canon
                        ));
                    }
                    None => {
                        url_bad += 1;
                        remote_token = Some("no-origin".yellow().to_string());
                        remote_notes.push(format!(
                            "  {} {}: no 'origin' remote — expected {}",
                            "✗".red(),
                            repo.name,
                            canon
                        ));
                    }
                }
            }
        }

        let identity = verify::check(cfg, repo);
        // (visible length, colored token) — visible length is used for padding
        // since the colored string carries invisible ANSI escapes.
        let (id_len, id_col) = if !identity.applies {
            (1, "—".dimmed().to_string())
        } else {
            match identity.worst() {
                State::Pass => {
                    id_pass += 1;
                    (4, "id ✓".green().to_string())
                }
                State::Warn => {
                    id_pass += 1;
                    (4, "id !".yellow().to_string())
                }
                State::Fail => {
                    id_fail += 1;
                    (4, "id ✗".red().to_string())
                }
            }
        };
        let id_pad = " ".repeat(4usize.saturating_sub(id_len));

        let git_col = if !dir.is_dir() {
            "missing".red().to_string()
        } else if !g.is_repo {
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
            if let Some(tok) = &remote_token {
                parts.push(tok.clone());
            }
            parts.join(" ")
        };

        let ver = info.version.as_deref().unwrap_or("—");

        println!(
            "  {:<name_w$}  {:<5}  {:<10}  {}{}  {}",
            repo.name.bold(),
            info.lang.to_string(),
            ver,
            id_col,
            id_pad,
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

    if id_pass + id_fail > 0 {
        if id_fail == 0 {
            println!("{} {id_pass} repo(s) pass", "identity:".green().bold());
        } else {
            println!(
                "{} {id_fail} repo(s) fail (run `basis verify` for details)",
                "identity:".red().bold()
            );
        }
    }

    if url_ok + url_bad > 0 {
        if url_bad == 0 {
            println!(
                "{} {url_ok} repo(s) match canonical URL",
                "remotes:".green().bold()
            );
        } else {
            println!(
                "{} {url_bad} repo(s) differ from canonical URL",
                "remotes:".red().bold()
            );
            for note in &remote_notes {
                println!("{note}");
            }
        }
    }

    let displays = display::statuses(cfg);
    if !displays.is_empty() {
        println!();
        let dname_w = displays
            .iter()
            .map(|d| d.name.len())
            .max()
            .unwrap_or(7)
            .max(7);
        println!("{}", "displays:".bold());
        for d in &displays {
            let state = if d.running {
                "● running".green().to_string()
            } else {
                "○ stopped".dimmed().to_string()
            };
            println!(
                "  {:<dname_w$}  {}  {} pane(s), {}  {}",
                d.name.bold(),
                state,
                d.panes,
                d.layout,
                format!("[{}]", d.session).dimmed(),
                dname_w = dname_w
            );
        }
    }

    Ok(())
}
