mod crate_dependencies;
mod migration_history;
mod source_size;

use std::path::PathBuf;

use anyhow::{Context, Result};

pub(crate) fn run() -> Result<()> {
    let workspace = workspace_root()?;

    crate_dependencies::check(&workspace)?;
    migration_history::check(&workspace)?;
    source_size::check(&workspace)?;
    println!("architecture checks passed");
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .context("xtask must live directly under the workspace root")
}
