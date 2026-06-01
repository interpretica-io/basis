mod cli;
mod config;
mod display;
mod git;
mod gpg;
mod install;
mod runner;
mod status;
mod verify;
mod version;

use std::ffi::OsString;

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use cli::{ActionArgs, Cli, Command, VersionCommand};

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
        Command::Action(tokens) => {
            // tokens[0] is the action name; the rest are its flags.
            let argv = std::iter::once(OsString::from("basis"))
                .chain(tokens.into_iter().map(OsString::from));
            let inv = ActionArgs::try_parse_from(argv)?;
            runner::run_action(&load()?, &inv.action, &inv.args)
        }
        Command::Display {
            name,
            kill,
            detached,
        } => display::run(&load()?, name, kill, detached),
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
