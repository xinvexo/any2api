use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

const PUBLISHER_PATH: &str = "crates/runtime/src/configuration/publisher/config_publisher.rs";

pub(crate) fn check(workspace: &Path) -> Result<()> {
    check_directory(&workspace.join("crates"), workspace)
}

fn check_directory(directory: &Path, workspace: &Path) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            check_directory(&path, workspace)?;
        } else if is_production_rust(&path) {
            check_file(&path, workspace)?;
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

fn check_file(path: &Path, workspace: &Path) -> Result<()> {
    let relative = path
        .strip_prefix(workspace)
        .with_context(|| format!("source path is outside workspace: {}", path.display()))?;
    let relative = relative.to_string_lossy();
    if relative == PUBLISHER_PATH {
        return Ok(());
    }

    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    if let Some(line) = source
        .lines()
        .enumerate()
        .find_map(|(index, line)| is_direct_prepare_call(line).then_some(index + 1))
    {
        bail!(
            "configuration candidate transactions may only be consumed by ConfigPublisher: {}:{line}",
            path.display()
        );
    }
    Ok(())
}

fn is_direct_prepare_call(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.starts_with("//")
        && (line.contains(".prepare_configuration(") || line.contains("::prepare_configuration("))
}

#[cfg(test)]
mod tests {
    use super::is_direct_prepare_call;

    #[test]
    fn recognizes_direct_candidate_transaction_calls() {
        assert!(is_direct_prepare_call(
            "let prepared = store.prepare_configuration(expected, mutation).await?;"
        ));
        assert!(is_direct_prepare_call(
            "let prepared = <SqliteStore as ConfigurationRepository>::prepare_configuration(&store, expected, mutation);"
        ));
    }

    #[test]
    fn ignores_declarations_comments_and_other_mutations() {
        assert!(!is_direct_prepare_call(
            "async fn prepare_configuration(&self, mutation: Mutation) {}"
        ));
        assert!(!is_direct_prepare_call(
            "// store.prepare_configuration(expected, mutation)"
        ));
        assert!(!is_direct_prepare_call(
            "let prepared = store.prepare_configuration_mutation(expected, mutation).await?;"
        ));
    }
}
