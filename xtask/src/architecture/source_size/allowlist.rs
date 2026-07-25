use std::{collections::HashMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Deserialize)]
struct AllowlistDocument {
    #[serde(default)]
    exceptions: Vec<AllowlistEntry>,
}

#[derive(Debug, Deserialize)]
struct AllowlistEntry {
    path: String,
    reason: String,
    adr: String,
    owner: String,
    expires_at: String,
}

#[derive(Debug)]
pub(crate) struct Allowlist {
    entries: HashMap<String, AllowlistEntry>,
}

impl Allowlist {
    pub(crate) fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }
}

pub(crate) fn load(path: &Path) -> Result<Allowlist> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let document: AllowlistDocument =
        toml::from_str(&raw).context("invalid architecture allowlist")?;
    let today = current_utc_date();
    let mut entries = HashMap::new();

    for entry in document.exceptions {
        validate_entry(&entry, &today)?;
        if entries.insert(entry.path.clone(), entry).is_some() {
            bail!("duplicate architecture allowlist path");
        }
    }

    Ok(Allowlist { entries })
}

fn validate_entry(entry: &AllowlistEntry, today: &str) -> Result<()> {
    if entry.path.trim().is_empty()
        || entry.reason.trim().is_empty()
        || entry.adr.trim().is_empty()
        || entry.owner.trim().is_empty()
        || entry.expires_at.trim().is_empty()
    {
        bail!("allowlist entries require path, reason, adr, owner and expires_at");
    }
    if entry.expires_at.as_str() < today {
        bail!("architecture allowlist entry expired: {}", entry.path);
    }

    Ok(())
}

fn current_utc_date() -> String {
    OffsetDateTime::now_utc().date().to_string()
}
