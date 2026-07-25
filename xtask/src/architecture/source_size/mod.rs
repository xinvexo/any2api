use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};
use tokei::{Config, Languages};

use allowlist::Allowlist;

mod allowlist;

const HARD_LIMIT: usize = 600;
const ALLOWLIST_START: usize = 401;
const ROOT_MODULE_LIMIT: usize = 160;

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let allowlist = allowlist::load(&workspace.join("architecture-allowlist.toml"))?;
    let roots = source_roots(workspace);
    let mut languages = Languages::new();
    languages.get_statistics(
        &roots,
        &["target", "node_modules", "reference"],
        &Config::default(),
    );
    let mut scanned_paths = BTreeSet::new();
    let mut required_paths = BTreeSet::new();

    for language in languages.values() {
        for report in &language.reports {
            let relative = relative_path(workspace, report.name.as_path());
            scanned_paths.insert(relative.clone());
            if report.stats.code >= ALLOWLIST_START {
                required_paths.insert(relative.clone());
            }
            check_report(
                report.name.as_path(),
                &relative,
                report.stats.code,
                &allowlist,
            )?;
        }
    }

    allowlist.validate_usage(&scanned_paths, &required_paths)
}

fn check_report(
    path: &Path,
    relative: &str,
    code_lines: usize,
    allowlist: &Allowlist,
) -> Result<()> {
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
    if code_lines >= ALLOWLIST_START && !allowlist.contains(relative) {
        bail!("source file requires architecture allowlist entry: {relative}");
    }

    Ok(())
}

fn relative_path(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn source_roots(workspace: &Path) -> Vec<PathBuf> {
    ["crates", "app", "web/src", "xtask/src"]
        .into_iter()
        .map(|path| workspace.join(path))
        .filter(|path| path.exists())
        .collect()
}
