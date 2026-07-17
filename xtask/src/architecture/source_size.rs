use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use tokei::{Config, Languages};

use super::allowlist::Allowlist;

const HARD_LIMIT: usize = 600;
const ALLOWLIST_START: usize = 401;
const ROOT_MODULE_LIMIT: usize = 160;

pub(crate) fn check(workspace: &Path, allowlist: &Allowlist) -> Result<()> {
    let roots = source_roots(workspace);
    let mut languages = Languages::new();
    languages.get_statistics(
        &roots,
        &["target", "node_modules", "reference"],
        &Config::default(),
    );

    for language in languages.values() {
        for report in &language.reports {
            check_report(
                workspace,
                report.name.as_path(),
                report.stats.code,
                allowlist,
            )?;
        }
    }

    Ok(())
}

fn check_report(
    workspace: &Path,
    path: &Path,
    code_lines: usize,
    allowlist: &Allowlist,
) -> Result<()> {
    let relative = path
        .strip_prefix(workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");

    if matches!(file_name, "main.rs" | "lib.rs" | "mod.rs") && code_lines > ROOT_MODULE_LIMIT {
        bail!("root module exceeds {ROOT_MODULE_LIMIT} code lines: {relative}");
    }
    if code_lines > HARD_LIMIT {
        bail!("source file exceeds {HARD_LIMIT} code lines: {relative}");
    }
    if code_lines >= ALLOWLIST_START && !allowlist.contains(&relative) {
        bail!("source file requires architecture allowlist entry: {relative}");
    }

    Ok(())
}

fn source_roots(workspace: &Path) -> Vec<PathBuf> {
    ["crates", "app", "web/src", "xtask/src"]
        .into_iter()
        .map(|path| workspace.join(path))
        .filter(|path| path.exists())
        .collect()
}
