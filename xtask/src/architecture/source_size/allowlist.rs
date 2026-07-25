use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

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
    entries: BTreeMap<String, AllowlistEntry>,
}

impl Allowlist {
    pub(crate) fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    pub(crate) fn validate_usage(
        &self,
        scanned_paths: &BTreeSet<String>,
        required_paths: &BTreeSet<String>,
    ) -> Result<()> {
        for path in self.entries.keys() {
            if !scanned_paths.contains(path) {
                bail!("architecture allowlist entry does not match a scanned source file: {path}");
            }
            if !required_paths.contains(path) {
                bail!("architecture allowlist entry is no longer required: {path}");
            }
        }

        Ok(())
    }
}

pub(crate) fn load(path: &Path) -> Result<Allowlist> {
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let document: AllowlistDocument =
        toml::from_str(&raw).context("invalid architecture allowlist")?;
    let today = current_utc_date();
    let mut entries = BTreeMap::new();

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

#[cfg(test)]
mod tests {
    use super::*;

    const PATH: &str = "crates/runtime/src/oauth/quota.rs";

    #[test]
    fn rejects_entry_that_does_not_match_scanned_source() {
        let error = allowlist().validate_usage(&BTreeSet::new(), &BTreeSet::new());

        assert_eq!(
            error.unwrap_err().to_string(),
            "architecture allowlist entry does not match a scanned source file: \
             crates/runtime/src/oauth/quota.rs"
        );
    }

    #[test]
    fn rejects_entry_that_no_longer_requires_exception() {
        let scanned_paths = BTreeSet::from([PATH.to_owned()]);
        let error = allowlist().validate_usage(&scanned_paths, &BTreeSet::new());

        assert_eq!(
            error.unwrap_err().to_string(),
            "architecture allowlist entry is no longer required: \
             crates/runtime/src/oauth/quota.rs"
        );
    }

    #[test]
    fn accepts_entry_that_is_scanned_and_required() {
        let paths = BTreeSet::from([PATH.to_owned()]);

        allowlist().validate_usage(&paths, &paths).unwrap();
    }

    fn allowlist() -> Allowlist {
        Allowlist {
            entries: BTreeMap::from([(
                PATH.to_owned(),
                AllowlistEntry {
                    path: PATH.to_owned(),
                    reason: "cohesive fixture".to_owned(),
                    adr: "docs/adr/0047-feature-first-crate-layout.md".to_owned(),
                    owner: "maintainer".to_owned(),
                    expires_at: "2099-12-31".to_owned(),
                },
            )]),
        }
    }
}
