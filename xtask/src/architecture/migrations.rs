use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
struct ChecksumDocument {
    migrations: BTreeMap<String, String>,
}

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let migration_dir = workspace.join("migrations");
    let checksum_path = migration_dir.join("checksums.toml");
    let checksum_raw = fs::read_to_string(&checksum_path)
        .with_context(|| format!("failed to read {}", checksum_path.display()))?;
    let checksums: ChecksumDocument =
        toml::from_str(&checksum_raw).context("invalid migration checksum file")?;
    let mut files = fs::read_dir(&migration_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect::<Vec<_>>();
    files.sort();

    for (index, path) in files.iter().enumerate() {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("migration filename must be valid UTF-8")?;
        let expected_prefix = format!("{:04}_", index + 1);
        if !file_name.starts_with(&expected_prefix) {
            bail!("migration sequence must be contiguous: {file_name}");
        }

        let expected = checksums
            .migrations
            .get(file_name)
            .with_context(|| format!("missing checksum for {file_name}"))?;
        let actual = hex_sha256(&fs::read(path)?);
        if &actual != expected {
            bail!("migration checksum mismatch: {file_name}");
        }
    }

    if checksums.migrations.len() != files.len() {
        bail!("migration checksum file contains stale entries");
    }

    Ok(())
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
