use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Constellation build system: build, version and sync groups of repositories.
#[derive(Parser, Debug)]
#[command(name = "basis", version, about, long_about = None)]
pub struct Cli {
    /// Path to the constellation manifest.
    #[arg(short, long, global = true, default_value = "basis.yaml")]
    pub file: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Clone a constellation: fetch its manifest repo, then every member repo.
    Install {
        /// Manifest repository: `org/repo` (GitHub) or a full git URL.
        spec: String,
        /// Directory to create for the constellation (default: the repo name).
        #[arg(long)]
        into: Option<PathBuf>,
        /// Branch to check out for the manifest repository.
        #[arg(long)]
        branch: Option<String>,
    },
    /// Launch (or manage) a tmux display: one pane per task.
    Display {
        /// Display name. Omit to list configured displays.
        name: Option<String>,
        /// Kill the display's tmux session instead of starting it.
        #[arg(long)]
        kill: bool,
        /// Create the session but do not attach to it.
        #[arg(long)]
        detached: bool,
    },
    /// Show git and version status of every repository.
    Status,
    /// Verify git/GPG identity e-mail domains against the manifest policy.
    Verify,
    /// Inspect or change versions across the constellation.
    Version {
        #[command(subcommand)]
        cmd: Option<VersionCommand>,
    },
    /// Any other name runs the matching `action` across the constellation,
    /// e.g. `basis build`, `basis test`, `basis run`. (Captured verbatim;
    /// the first token is the action, the rest are parsed as `ActionArgs`.)
    #[command(external_subcommand)]
    Action(Vec<String>),
}

/// Parsed form of `basis <action> [flags]` (a non-reserved subcommand).
#[derive(Parser, Debug)]
pub struct ActionArgs {
    /// Action name (key under a repo's `actions`).
    pub action: String,
    #[command(flatten)]
    pub args: RunArgs,
}

#[derive(Args, Debug, Default)]
pub struct RunArgs {
    /// Only operate on these repositories (by name). Repeatable.
    #[arg(short, long = "repo")]
    pub repos: Vec<String>,

    /// Keep going across repositories even if one command fails.
    #[arg(short = 'k', long)]
    pub keep_going: bool,

    /// Print what would run without executing.
    #[arg(short = 'n', long)]
    pub dry_run: bool,

    /// Force running the action in a per-task tmux display (overrides the
    /// manifest's `tmux:` setting for this run).
    #[arg(short = 't', long)]
    pub tmux: bool,

    /// Force running in the current terminal, even if the manifest enables tmux.
    #[arg(long, conflicts_with = "tmux")]
    pub no_tmux: bool,

    /// With tmux: create the session but do not attach.
    #[arg(long)]
    pub detached: bool,

    /// With tmux: tmux layout to apply (default: tiled).
    #[arg(long)]
    pub layout: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum VersionCommand {
    /// List the current version of every repository (default).
    Show,
    /// Set an explicit version on every repository.
    Set {
        /// Semver version, e.g. 1.2.3.
        version: String,
    },
    /// Synchronise all repositories to a single version.
    Sync {
        /// Target version. Defaults to the manifest `version`,
        /// otherwise the highest version found among repositories.
        #[arg(long)]
        to: Option<String>,
    },
    /// Bump one component's version and update repos that depend on it.
    Bump {
        /// Repository (component) to bump.
        repo: String,
        /// Increment the major version (X.0.0).
        #[arg(long, group = "how")]
        major: bool,
        /// Increment the minor version (x.Y.0).
        #[arg(long, group = "how")]
        minor: bool,
        /// Increment the patch version (x.y.Z). This is the default.
        #[arg(long, group = "how")]
        patch: bool,
        /// Set an explicit target version instead of incrementing.
        #[arg(long, group = "how")]
        to: Option<String>,
    },
}
