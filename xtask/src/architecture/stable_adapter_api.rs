use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

const ADAPTER_CRATES: [&str; 4] = [
    "any2api_protocol",
    "any2api_provider",
    "any2api_storage",
    "any2api_transport",
];

pub(crate) fn check(workspace: &Path) -> Result<()> {
    check_directory(&workspace.join("crates/runtime/src"))
}

fn check_directory(directory: &Path) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            check_directory(&path)?;
        } else if is_production_rust(&path) {
            check_file(&path)?;
        }
    }
    Ok(())
}

fn is_production_rust(path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "rs") {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name != "tests.rs" && name != "test_support.rs" && !name.ends_with("_tests.rs")
}

fn check_file(path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if let Some((line, adapter)) = forbidden_import(&source) {
        bail!(
            "Runtime must import {adapter} only through its stable api module: {}:{line}",
            path.display()
        );
    }
    Ok(())
}

fn forbidden_import(source: &str) -> Option<(usize, &'static str)> {
    for (index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for adapter in ADAPTER_CRATES {
            let prefix = format!("{adapter}::");
            if line
                .match_indices(&prefix)
                .any(|(offset, _)| !line[offset + prefix.len()..].starts_with("api::"))
            {
                return Some((index + 1, adapter));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_stable_api_imports() {
        assert_eq!(
            forbidden_import("use any2api_provider::api::{ProviderDriver, ProviderRegistry};"),
            None
        );
    }

    #[test]
    fn rejects_root_and_mixed_imports() {
        assert_eq!(
            forbidden_import("use any2api_protocol::{ProtocolError, api::ProtocolRegistry};"),
            Some((1, "any2api_protocol"))
        );
        assert_eq!(
            forbidden_import("let error = any2api_provider::ProviderError::InvalidResponse(x);"),
            Some((1, "any2api_provider"))
        );
    }
}
