mod cli;
mod config;
mod git;
mod gpg;
mod install;
mod runner;
mod status;
mod verify;
mod version;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{Cli, Command, VersionCommand};

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {:#}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let file = cli.file;
    // `install` fetches the manifest itself, so config is loaded lazily.
    let load = || config::Config::load(&file);

    match cli.command {
        Command::Install { spec, into, branch } => install::run(&spec, into, branch, &file),
        Command::Build(args) => runner::run_action(&load()?, "build", &args),
        Command::Clean(args) => runner::run_action(&load()?, "clean", &args),
        Command::Run { action, args } => runner::run_action(&load()?, &action, &args),
        Command::Status => status::show(&load()?),
        Command::Verify => verify::run(&load()?),
        Command::Version { cmd } => {
            let cfg = load()?;
            match cmd.unwrap_or(VersionCommand::Show) {
                VersionCommand::Show => version::show(&cfg),
                VersionCommand::Set { version } => version::set_all(&cfg, &version),
                VersionCommand::Sync { to } => version::sync(&cfg, to.as_deref()),
                VersionCommand::Bump {
                    repo,
                    major,
                    minor,
                    patch: _,
                    to,
                } => {
                    let how = if let Some(v) = to {
                        version::Bump::To(v)
                    } else if major {
                        version::Bump::Major
                    } else if minor {
                        version::Bump::Minor
                    } else {
                        version::Bump::Patch
                    };
                    version::bump(&cfg, &repo, how)
                }
            }
        }
    }
}
