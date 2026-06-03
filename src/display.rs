use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use colored::Colorize;

use crate::cli::RunArgs;
use crate::config::{Config, Display};

/// A pane resolved to a concrete working directory and its commands. Each
/// command is sent to the pane on its own line (no `&&` joining).
struct ResolvedPane {
    title: String,
    dir: PathBuf,
    commands: Vec<String>,
}

/// Summary of one display for `basis status`.
pub struct DisplayStatus {
    pub name: String,
    pub panes: usize,
    pub layout: String,
    pub session: String,
    pub running: bool,
}

/// Summarise every configured display and whether its tmux session is up.
pub fn statuses(cfg: &Config) -> Vec<DisplayStatus> {
    cfg.manifest
        .displays
        .iter()
        .map(|(name, display)| {
            let session = session_name(cfg, name, display);
            let running = session_exists(&session);
            DisplayStatus {
                name: name.clone(),
                panes: display.panes.len(),
                layout: display.layout.clone(),
                session,
                running,
            }
        })
        .collect()
}

/// Entry point for `basis display [name] [--kill] [--detached]`.
pub fn run(cfg: &Config, name: Option<String>, kill: bool, detached: bool) -> Result<()> {
    let Some(name) = name else {
        return list(cfg);
    };

    let display = cfg
        .manifest
        .displays
        .get(&name)
        .with_context(|| format!("unknown display '{name}'"))?;
    let session = session_name(cfg, &name, display);

    ensure_tmux()?;

    if kill {
        return kill_session(&session);
    }

    let panes = resolve_panes(cfg, display)?;
    if panes.is_empty() {
        bail!("display '{name}' has no panes");
    }
    up_and_attach(&session, &name, &panes, &display.layout, detached)
}

/// Resolve the display (tmux session) name for `action`, or `None` to run it
/// inline. `--no-tmux` forces inline; the task's `display:` name wins next;
/// `--tmux` forces a display named `<constellation>-<action>`.
pub fn resolve_session(cfg: &Config, action: &str, args: &RunArgs) -> Option<String> {
    if args.no_tmux {
        return None;
    }
    if let Some(name) = cfg.task_display(action) {
        return Some(sanitize(&name));
    }
    if args.tmux {
        return Some(sanitize(&format!(
            "{}-{}",
            cfg.manifest.constellation, action
        )));
    }
    None
}

/// Create a tmux display for a *task*: run `action` in one pane per selected
/// repository (in parallel), then attach. This is the dynamic, per-task display.
pub fn launch_task(cfg: &Config, action: &str, args: &RunArgs, session: &str) -> Result<()> {
    ensure_tmux()?;

    let mut panes = Vec::new();
    for repo in cfg.select(&args.repos)? {
        let Some(cmds) = repo.actions.get(action) else {
            continue;
        };
        panes.push(ResolvedPane {
            title: repo.name.clone(),
            dir: cfg.repo_dir(repo),
            commands: cmds.clone(),
        });
    }
    if panes.is_empty() {
        bail!("no selected repository defines a '{action}' action");
    }

    // Layout: --layout overrides the per-task config, default tiled.
    let layout = args
        .layout
        .clone()
        .or_else(|| cfg.task_layout(action))
        .unwrap_or_else(|| "tiled".to_string());
    up_and_attach(session, action, &panes, &layout, args.detached)
}

/// Ensure a session exists (building it from `panes` if needed) and attach
/// unless `detached`.
fn up_and_attach(
    session: &str,
    window: &str,
    panes: &[ResolvedPane],
    layout: &str,
    detached: bool,
) -> Result<()> {
    if session_exists(session) {
        println!(
            "{} session {} already running",
            "·".dimmed(),
            session.cyan()
        );
    } else {
        build_session(session, window, panes, layout)?;
        println!(
            "{} tmux session {} with {} pane(s)",
            "created".green().bold(),
            session.cyan(),
            panes.len()
        );
    }

    if detached {
        println!(
            "attach with: {}",
            format!("tmux attach -t {session}").bold()
        );
        return Ok(());
    }
    attach(session)
}

/// List the displays configured in the manifest.
fn list(cfg: &Config) -> Result<()> {
    if cfg.manifest.displays.is_empty() {
        println!("no displays configured (add a `displays:` section to the manifest)");
        return Ok(());
    }
    println!("{}", "displays:".bold());
    for (name, display) in &cfg.manifest.displays {
        println!(
            "  {:<16} {} pane(s), layout {}",
            name.bold(),
            display.panes.len(),
            display.layout
        );
    }
    Ok(())
}

/// Build a tmux session with one pane per `ResolvedPane`, applying `layout`.
fn build_session(session: &str, window: &str, panes: &[ResolvedPane], layout: &str) -> Result<()> {
    let target = format!("{session}:{window}");

    // First pane creates the session+window; capture its pane id.
    let mut pane_ids = Vec::new();
    pane_ids.push(tmux_capture(&[
        "new-session",
        "-d",
        "-P",
        "-F",
        "#{pane_id}",
        "-s",
        session,
        "-n",
        window,
        "-c",
        &dir_arg(&panes[0]),
    ])?);

    // Remaining panes split the window; re-tile each time to keep room.
    for pane in &panes[1..] {
        let id = tmux_capture(&[
            "split-window",
            "-P",
            "-F",
            "#{pane_id}",
            "-t",
            &target,
            "-c",
            &dir_arg(pane),
        ])?;
        pane_ids.push(id);
        tmux(&["select-layout", "-t", &target, layout])?;
    }
    tmux(&["select-layout", "-t", &target, layout])?;

    // Show pane titles along the borders.
    tmux(&["set-option", "-t", session, "pane-border-status", "top"]).ok();

    // Ctrl-q (no prefix) closes the display: kill the current session and the
    // processes running in it.
    tmux(&["bind-key", "-n", "C-q", "kill-session"]).ok();

    for (pane, id) in panes.iter().zip(&pane_ids) {
        let id = id.trim();
        tmux(&["select-pane", "-t", id, "-T", &pane.title]).ok();
        // Send each command on its own line (no `&&` joining). Run each via
        // `sh -c` so commands are POSIX-interpreted — same as inline actions —
        // regardless of the user's interactive shell (fish, zsh, ...).
        for cmd in &pane.commands {
            tmux(&["send-keys", "-t", id, &sh_wrap(cmd), "Enter"])?;
        }
    }
    Ok(())
}

/// Wrap a command so a pane runs it via `sh -c`, independent of the user's
/// interactive shell. Single quotes in the command are escaped.
fn sh_wrap(cmd: &str) -> String {
    let escaped = cmd.replace('\'', r"'\''");
    format!("sh -c '{escaped}'")
}

/// Resolve every pane's working directory and command.
fn resolve_panes(cfg: &Config, display: &Display) -> Result<Vec<ResolvedPane>> {
    let mut out = Vec::new();
    for pane in &display.panes {
        // Working directory: explicit cwd > repo dir > base dir.
        let dir = if let Some(cwd) = &pane.cwd {
            cfg.base_dir.join(cwd)
        } else if let Some(repo) = &pane.repo {
            cfg.repo_dir(cfg.find_repo(repo)?)
        } else {
            cfg.base_dir.clone()
        };

        // Commands: explicit command > named repo action > none (plain shell).
        let commands = if let Some(cmd) = &pane.command {
            vec![cmd.clone()]
        } else if let Some(action) = &pane.action {
            let repo_name = pane
                .repo
                .as_ref()
                .with_context(|| format!("pane action '{action}' requires a `repo`"))?;
            let repo = cfg.find_repo(repo_name)?;
            repo.actions
                .get(action)
                .with_context(|| format!("repo '{repo_name}' has no '{action}' action"))?
                .clone()
        } else {
            Vec::new()
        };

        let title = pane
            .name
            .clone()
            .or_else(|| pane.repo.clone())
            .or_else(|| pane.action.clone())
            .unwrap_or_else(|| "shell".to_string());

        out.push(ResolvedPane {
            title,
            dir,
            commands,
        });
    }
    Ok(out)
}

fn dir_arg(pane: &ResolvedPane) -> String {
    pane.dir.to_string_lossy().to_string()
}

/// Default session name when the display does not set one.
fn session_name(cfg: &Config, name: &str, display: &Display) -> String {
    let raw = display
        .session
        .clone()
        .unwrap_or_else(|| format!("{}-{}", cfg.manifest.constellation, name));
    sanitize(&raw)
}

/// tmux session names may not contain '.', ':' or spaces.
fn sanitize(name: &str) -> String {
    name.replace(['.', ':', ' '], "-")
}

fn attach(session: &str) -> Result<()> {
    // Nested attach is refused by tmux; guide the user instead.
    if std::env::var_os("TMUX").is_some() {
        println!(
            "already inside tmux — switch with: {}",
            format!("tmux switch-client -t {session}").bold()
        );
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        println!(
            "not a terminal — attach with: {}",
            format!("tmux attach -t {session}").bold()
        );
        return Ok(());
    }
    let status = Command::new("tmux")
        .args(["attach", "-t", session])
        .status()
        .context("failed to run tmux attach")?;
    if !status.success() {
        bail!("tmux attach exited with {status}");
    }
    Ok(())
}

fn kill_session(session: &str) -> Result<()> {
    if !session_exists(session) {
        println!("{} session {} is not running", "·".dimmed(), session);
        return Ok(());
    }
    tmux(&["kill-session", "-t", session])?;
    println!("{} session {}", "killed".red().bold(), session);
    Ok(())
}

fn session_exists(session: &str) -> bool {
    // Silence "error connecting ..." when no tmux server is running yet.
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn ensure_tmux() -> Result<()> {
    let ok = Command::new("tmux")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        bail!("tmux not found in PATH — install tmux to use displays");
    }
    Ok(())
}

/// Run a tmux command, failing on non-zero exit.
fn tmux(args: &[&str]) -> Result<()> {
    let status = Command::new("tmux")
        .args(args)
        .status()
        .context("failed to run tmux")?;
    if !status.success() {
        bail!("tmux {} exited with {status}", args.join(" "));
    }
    Ok(())
}

/// Run a tmux command and capture its trimmed stdout.
fn tmux_capture(args: &[&str]) -> Result<String> {
    let out = Command::new("tmux")
        .args(args)
        .output()
        .context("failed to run tmux")?;
    if !out.status.success() {
        bail!(
            "tmux {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
