mod allowlist;
mod dependencies;
mod migrations;
mod source_size;

use std::path::PathBuf;

use anyhow::{Context, Result};

pub(crate) fn run() -> Result<()> {
    let workspace = workspace_root()?;
    let allowlist = allowlist::load(&workspace.join("architecture-allowlist.toml"))?;

    dependencies::check(&workspace)?;
    migrations::check(&workspace)?;
    source_size::check(&workspace, &allowlist)?;
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
