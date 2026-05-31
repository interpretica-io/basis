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
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the `build` action across the constellation.
    Build(RunArgs),
    /// Run the `clean` action across the constellation.
    Clean(RunArgs),
    /// Run an arbitrary named action defined in the manifest.
    Run {
        /// Action name (key under a repo's `actions`).
        action: String,
        #[command(flatten)]
        args: RunArgs,
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
