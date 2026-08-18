use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use time::{Date, OffsetDateTime};

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

fn validate_entry(entry: &AllowlistEntry, today: &Date) -> Result<()> {
    if entry.path.trim().is_empty()
        || entry.reason.trim().is_empty()
        || entry.adr.trim().is_empty()
        || entry.owner.trim().is_empty()
        || entry.expires_at.trim().is_empty()
    {
        bail!("allowlist entries require path, reason, adr, owner and expires_at");
    }
    let expires_at = parse_expiry_date(&entry.expires_at).with_context(|| {
        format!(
            "architecture allowlist entry has invalid expires_at: {}",
            entry.path
        )
    })?;
    if expires_at < *today {
        bail!("architecture allowlist entry expired: {}", entry.path);
    }

    Ok(())
}

fn parse_expiry_date(value: &str) -> Result<Date> {
    let format = time::format_description::parse_borrowed::<3>("[year]-[month]-[day]")
        .context("invalid built-in allowlist date format")?;
    let date = Date::parse(value, &format).context("expected YYYY-MM-DD")?;
    if date.to_string() != value {
        bail!("expected canonical YYYY-MM-DD");
    }
    Ok(date)
}

fn current_utc_date() -> Date {
    OffsetDateTime::now_utc().date()
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

    #[test]
    fn rejects_noncanonical_or_impossible_expiry_dates() {
        let today = date("2026-08-03");
        for expires_at in ["never", "9999", "2026-8-03", "2026-02-30"] {
            let error =
                validate_entry(&entry(expires_at), &today).expect_err("invalid expiry must fail");
            assert!(
                error.to_string().contains("invalid expires_at"),
                "{expires_at}: {error}"
            );
        }
    }

    #[test]
    fn rejects_elapsed_expiry_but_accepts_today_and_future_dates() {
        let today = date("2026-08-03");
        let error =
            validate_entry(&entry("2026-08-02"), &today).expect_err("elapsed expiry must fail");
        assert_eq!(
            error.to_string(),
            "architecture allowlist entry expired: crates/runtime/src/oauth/quota.rs"
        );
        validate_entry(&entry("2026-08-03"), &today).expect("today");
        validate_entry(&entry("2026-08-04"), &today).expect("future date");
    }

    fn allowlist() -> Allowlist {
        Allowlist {
            entries: BTreeMap::from([(PATH.to_owned(), entry("2099-12-31"))]),
        }
    }

    fn entry(expires_at: &str) -> AllowlistEntry {
        AllowlistEntry {
            path: PATH.to_owned(),
            reason: "cohesive fixture".to_owned(),
            adr: "docs/adr/0170-current-decision-register.md".to_owned(),
            owner: "maintainer".to_owned(),
            expires_at: expires_at.to_owned(),
        }
    }

    fn date(value: &str) -> Date {
        parse_expiry_date(value).expect("test date")
    }
}
