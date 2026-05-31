mod cli;
mod config;
mod git;
mod runner;
mod status;
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
    let cfg = config::Config::load(&cli.file)?;

    match cli.command {
        Command::Build(args) => runner::run_action(&cfg, "build", &args),
        Command::Clean(args) => runner::run_action(&cfg, "clean", &args),
        Command::Run { action, args } => runner::run_action(&cfg, &action, &args),
        Command::Status => status::show(&cfg),
        Command::Version { cmd } => match cmd.unwrap_or(VersionCommand::Show) {
            VersionCommand::Show => version::show(&cfg),
            VersionCommand::Set { version } => version::set_all(&cfg, &version),
            VersionCommand::Sync { to } => version::sync(&cfg, to.as_deref()),
        },
    }
}
