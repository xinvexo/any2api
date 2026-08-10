use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

mod schema;

const BASELINE_DIRECTORY: &str = "docs/baselines/official-clients";

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let directory = workspace.join(BASELINE_DIRECTORY);
    let mut files = fs::read_dir(&directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    files.sort();
    if files.is_empty() {
        bail!("official client baseline directory contains no JSON fixtures");
    }

    for path in files {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        reject_raw_capture_material(&raw)
            .with_context(|| format!("unsafe official client baseline: {}", path.display()))?;
        schema::validate_document(&raw)
            .with_context(|| format!("invalid official client baseline: {}", path.display()))?;
    }
    Ok(())
}

fn reject_raw_capture_material(raw: &str) -> Result<()> {
    for forbidden in ["baseline-fixture-token", "Bearer ", "/Users/", "/tmp/"] {
        if raw.contains(forbidden) {
            bail!("baseline contains raw capture material");
        }
    }
    if contains_uuid_like(raw) {
        bail!("baseline contains a raw UUID-like identifier");
    }
    Ok(())
}

fn contains_uuid_like(value: &str) -> bool {
    value.as_bytes().windows(36).any(|candidate| {
        candidate
            .iter()
            .enumerate()
            .all(|(index, byte)| match index {
                8 | 13 | 18 | 23 => *byte == b'-',
                _ => byte.is_ascii_hexdigit(),
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{contains_uuid_like, reject_raw_capture_material};

    #[test]
    fn rejects_raw_capture_identifiers() {
        assert!(reject_raw_capture_material("Bearer secret").is_err());
        assert!(contains_uuid_like("00000000-1111-4222-8333-444444444444"));
        assert!(!contains_uuid_like("<dynamic:credential-owned>"));
    }
}
