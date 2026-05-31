use anyhow::{bail, Result};
use colored::Colorize;

use crate::config::{Config, Repo};
use crate::{git, gpg};

/// Outcome of a single identity check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Pass,
    /// Unverifiable, but not a policy violation (e.g. SSH-signed commits).
    Warn,
    Fail,
}

/// One named check (git e-mail, gpg key, ...) with a human-readable detail.
pub struct Check {
    pub state: State,
    pub label: &'static str,
    pub detail: String,
}

/// The identity verdict for a repository.
pub struct Identity {
    /// Whether an e-mail-domain policy applies to this repo at all.
    pub applies: bool,
    pub domains: Vec<String>,
    pub checks: Vec<Check>,
}

impl Identity {
    /// The most severe state among the checks (Pass < Warn < Fail).
    pub fn worst(&self) -> State {
        if self.checks.iter().any(|c| c.state == State::Fail) {
            State::Fail
        } else if self.checks.iter().any(|c| c.state == State::Warn) {
            State::Warn
        } else {
            State::Pass
        }
    }

    /// True if no check failed (warnings are tolerated).
    pub fn ok(&self) -> bool {
        self.worst() != State::Fail
    }
}

/// Compute the identity verdict for a repo without printing anything.
pub fn check(cfg: &Config, repo: &Repo) -> Identity {
    let domains = cfg.allowed_domains(repo);
    if domains.is_empty() {
        return Identity {
            applies: false,
            domains,
            checks: Vec::new(),
        };
    }

    let dir = cfg.repo_dir(repo);
    let checks = vec![check_git_email(&dir, &domains), check_gpg_key(&dir, &domains)];
    Identity {
        applies: true,
        domains,
        checks,
    }
}

/// Run identity verification across the constellation and print a report.
pub fn run(cfg: &Config) -> Result<()> {
    let mut checked = 0usize;
    let mut failures = 0usize;

    for repo in &cfg.manifest.repos {
        let id = check(cfg, repo);
        println!("\n{} {}", "==>".blue().bold(), repo.name.bold());

        if !id.applies {
            println!("  {} no e-mail domain configured — skipped", "·".dimmed());
            continue;
        }
        checked += 1;
        println!("  {} {}", "allowed domains:".dimmed(), id.domains.join(", "));
        for c in &id.checks {
            print_check(c);
        }
        if id.ok() {
            println!("  {} ok", "✓".green().bold());
        } else {
            failures += 1;
        }
    }

    println!();
    if failures > 0 {
        bail!("{failures} repo(s) failed identity verification");
    }
    if checked == 0 {
        println!(
            "{} nothing to verify — set `email_domain:` in the manifest",
            "note:".yellow()
        );
    } else {
        println!("{} {checked} repo(s) passed", "ok:".green().bold());
    }
    Ok(())
}

fn check_git_email(dir: &std::path::Path, domains: &[String]) -> Check {
    match git::config(dir, "user.email") {
        Some(email) => {
            let state = if email_on_domain(&email, domains) {
                State::Pass
            } else {
                State::Fail
            };
            Check {
                state,
                label: "git email",
                detail: email,
            }
        }
        None => Check {
            state: State::Fail,
            label: "git email",
            detail: "unset (git config user.email)".into(),
        },
    }
}

fn check_gpg_key(dir: &std::path::Path, domains: &[String]) -> Check {
    // SSH-signed commits carry no e-mail; we can only verify OpenPGP keys.
    let format = git::config(dir, "gpg.format").unwrap_or_else(|| "openpgp".into());
    if format == "ssh" {
        return Check {
            state: State::Warn,
            label: "gpg key",
            detail: "gpg.format=ssh, cannot verify key e-mail domain".into(),
        };
    }

    // Prefer the configured signing key; fall back to the git e-mail.
    let query = git::config(dir, "user.signingkey").or_else(|| git::config(dir, "user.email"));
    let Some(query) = query else {
        return Check {
            state: State::Fail,
            label: "gpg key",
            detail: "no user.signingkey or user.email to locate a key".into(),
        };
    };

    let lookup = gpg::secret_key_emails(&query);
    let (state, detail) = if lookup.gpg_missing {
        (State::Fail, "gpg binary not found in PATH".to_string())
    } else if !lookup.found {
        (State::Fail, format!("{query} — no secret key in keyring"))
    } else if lookup.emails.is_empty() {
        (State::Fail, format!("{query} — key has no e-mail user ID"))
    } else if lookup.emails.iter().any(|e| email_on_domain(e, domains)) {
        (State::Pass, format!("{query} [{}]", lookup.emails.join(", ")))
    } else {
        (State::Fail, format!("{query} [{}]", lookup.emails.join(", ")))
    };

    Check {
        state,
        label: "gpg key",
        detail,
    }
}

/// Whether `email`'s domain is one of the allowed domains (case-insensitive).
fn email_on_domain(email: &str, domains: &[String]) -> bool {
    match email.rsplit_once('@') {
        Some((_, d)) => domains.iter().any(|allowed| allowed == &d.to_lowercase()),
        None => false,
    }
}

fn print_check(c: &Check) {
    let mark = match c.state {
        State::Pass => "✓".green(),
        State::Warn => "!".yellow(),
        State::Fail => "✗".red(),
    };
    println!("  {mark} {}: {}", c.label, c.detail);
}
