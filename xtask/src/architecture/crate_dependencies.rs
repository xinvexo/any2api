use std::{collections::BTreeSet, path::Path};

use anyhow::{Result, bail};
use cargo_metadata::{MetadataCommand, Package};

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let metadata = MetadataCommand::new()
        .current_dir(workspace)
        .no_deps()
        .exec()?;
    let workspace_ids: BTreeSet<_> = metadata.workspace_members.iter().collect();

    for package in metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
    {
        check_package(package)?;
    }

    Ok(())
}

fn check_package(package: &Package) -> Result<()> {
    let allowed = allowed_dependencies(&package.name);

    for dependency in package
        .dependencies
        .iter()
        .filter(|dependency| is_any2api_workspace_dependency(&dependency.name))
    {
        if !allowed.contains(dependency.name.as_str()) {
            bail!(
                "forbidden workspace dependency: {} -> {}",
                package.name,
                dependency.name
            );
        }
    }

    Ok(())
}

fn is_any2api_workspace_dependency(name: &str) -> bool {
    name == "any2api" || name.starts_with("any2api-")
}

fn allowed_dependencies(package: &str) -> BTreeSet<&'static str> {
    match package {
        "any2api-domain" | "xtask" => BTreeSet::new(),
        "any2api-protocol" | "any2api-provider" | "any2api-transport" | "any2api-storage" => {
            BTreeSet::from(["any2api-domain"])
        }
        "any2api-runtime" => BTreeSet::from([
            "any2api-domain",
            "any2api-protocol",
            "any2api-provider",
            "any2api-storage",
            "any2api-transport",
        ]),
        "any2api-server" => BTreeSet::from(["any2api-domain", "any2api-runtime"]),
        "any2api" => BTreeSet::from([
            "any2api-domain",
            "any2api-protocol",
            "any2api-provider",
            "any2api-runtime",
            "any2api-server",
            "any2api-storage",
            "any2api-transport",
        ]),
        "any2api-contract-tests" => BTreeSet::from([
            "any2api",
            "any2api-domain",
            "any2api-protocol",
            "any2api-provider",
            "any2api-runtime",
            "any2api-server",
            "any2api-storage",
            "any2api-transport",
        ]),
        _ => BTreeSet::new(),
    }
}
