use std::{fs, path::Path};

use anyhow::{Context, Result, bail};

const PUBLISHER_PATH: &str = "crates/runtime/src/configuration/publisher/config_publisher.rs";
const STORAGE_API_PATH: &str = "crates/storage/src/api.rs";
const FORBIDDEN_STORAGE_TRANSACTION_EXPORTS: [&str; 3] = [
    "PreparedConfiguration",
    "ConfigurationCommit",
    "sqlx::Transaction",
];

pub(crate) fn check(workspace: &Path) -> Result<()> {
    check_directory(&workspace.join("crates"), workspace)?;
    check_storage_api(workspace)
}

fn check_storage_api(workspace: &Path) -> Result<()> {
    let path = workspace.join(STORAGE_API_PATH);
    let source =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if let Some(forbidden) = forbidden_storage_transaction_export(&source) {
        bail!("storage API exposes configuration transaction capability `{forbidden}`");
    }
    Ok(())
}

fn forbidden_storage_transaction_export(source: &str) -> Option<&'static str> {
    FORBIDDEN_STORAGE_TRANSACTION_EXPORTS
        .into_iter()
        .find(|forbidden| source.contains(forbidden))
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
        .find_map(|(index, line)| is_direct_transaction_call(line).then_some(index + 1))
    {
        bail!(
            "configuration candidate transactions may only be consumed by ConfigPublisher: {}:{line}",
            path.display()
        );
    }
    Ok(())
}

fn is_direct_transaction_call(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.starts_with("//")
        && (line.contains(".transact_configuration(") || line.contains("::transact_configuration("))
}

#[cfg(test)]
mod tests {
    use super::{forbidden_storage_transaction_export, is_direct_transaction_call};

    #[test]
    fn recognizes_direct_candidate_transaction_calls() {
        assert!(is_direct_transaction_call(
            "let outcome = store.transact_configuration(expected, mutation, compiler).await?;"
        ));
        assert!(is_direct_transaction_call(
            "let outcome = <SqliteStore as ConfigurationTransactionRepository>::transact_configuration(&store, expected, mutation, compiler);"
        ));
    }

    #[test]
    fn ignores_declarations_comments_and_other_mutations() {
        assert!(!is_direct_transaction_call(
            "async fn transact_configuration(&self, mutation: Mutation) {}"
        ));
        assert!(!is_direct_transaction_call(
            "// store.transact_configuration(expected, mutation, compiler)"
        ));
        assert!(!is_direct_transaction_call(
            "let outcome = store.transact_configuration_mutation(expected, mutation).await?;"
        ));
    }

    #[test]
    fn rejects_legacy_or_concrete_transaction_exports() {
        assert_eq!(
            forbidden_storage_transaction_export("pub use x::PreparedConfiguration;"),
            Some("PreparedConfiguration")
        );
        assert_eq!(
            forbidden_storage_transaction_export("pub use sqlx::Transaction;"),
            Some("sqlx::Transaction")
        );
        assert_eq!(
            forbidden_storage_transaction_export(
                "pub use x::{ConfigurationTransactionOutcome, ConfigurationRepository};"
            ),
            None
        );
    }
}
