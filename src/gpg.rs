use std::process::Command;

/// Result of locating a GPG signing key.
pub struct KeyLookup {
    /// True if `gpg` ran and reported a matching secret key.
    pub found: bool,
    /// E-mail addresses on the key's user IDs.
    pub emails: Vec<String>,
    /// True if the `gpg` binary itself could not be executed.
    pub gpg_missing: bool,
}

/// Look up a secret (signing-capable) key by id, fingerprint or e-mail and
/// return the e-mail addresses on its user IDs.
pub fn secret_key_emails(query: &str) -> KeyLookup {
    let output = Command::new("gpg")
        .args(["--list-secret-keys", "--with-colons", query])
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => o.stdout,
        Ok(_) => {
            return KeyLookup {
                found: false,
                emails: Vec::new(),
                gpg_missing: false,
            }
        }
        Err(_) => {
            return KeyLookup {
                found: false,
                emails: Vec::new(),
                gpg_missing: true,
            }
        }
    };

    let text = String::from_utf8_lossy(&stdout);
    let emails = parse_uid_emails(&text);
    // A `sec` record means a secret key was actually found.
    let found = text.lines().any(|l| l.starts_with("sec:"));

    KeyLookup {
        found,
        emails,
        gpg_missing: false,
    }
}

/// Extract e-mails from the `uid` records of `gpg --with-colons` output.
fn parse_uid_emails(text: &str) -> Vec<String> {
    let mut emails = Vec::new();
    for line in text.lines() {
        if !line.starts_with("uid:") {
            continue;
        }
        // The user-id string is the 10th colon-separated field (index 9).
        if let Some(uid) = line.split(':').nth(9) {
            if let Some(email) = extract_email(uid) {
                emails.push(email.to_lowercase());
            }
        }
    }
    emails.sort();
    emails.dedup();
    emails
}

/// Pull the `<addr>` out of a `Name (comment) <addr>` user-id string.
fn extract_email(uid: &str) -> Option<String> {
    let start = uid.find('<')?;
    let rest = &uid[start + 1..];
    let end = rest.find('>')?;
    Some(rest[..end].to_string())
}
