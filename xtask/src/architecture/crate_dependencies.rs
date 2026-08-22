use std::{collections::BTreeSet, path::Path};

use anyhow::{Result, bail};
use cargo_metadata::{MetadataCommand, Package};

const DEPENDENCY_POLICY: &[(&str, &[&str])] = &[
    ("any2api-domain", &[]),
    ("any2api-memory-reclaimer", &[]),
    ("any2api-payload-buffer", &[]),
    ("any2api-updater", &[]),
    ("xtask", &[]),
    (
        "any2api-protocol",
        &["any2api-domain", "any2api-payload-buffer"],
    ),
    (
        "any2api-provider",
        &[
            "any2api-domain",
            "any2api-payload-buffer",
            "any2api-protocol",
        ],
    ),
    ("any2api-transport", &["any2api-domain"]),
    ("any2api-storage", &["any2api-domain"]),
    (
        "any2api-runtime",
        &[
            "any2api-domain",
            "any2api-payload-buffer",
            "any2api-protocol",
            "any2api-provider",
            "any2api-storage",
            "any2api-transport",
        ],
    ),
    (
        "any2api-server",
        &[
            "any2api-domain",
            "any2api-payload-buffer",
            "any2api-runtime",
            "any2api-updater",
        ],
    ),
    (
        "any2api",
        &[
            "any2api-domain",
            "any2api-memory-reclaimer",
            "any2api-protocol",
            "any2api-provider",
            "any2api-runtime",
            "any2api-server",
            "any2api-storage",
            "any2api-transport",
            "any2api-updater",
        ],
    ),
    (
        "any2api-contract-tests",
        &[
            "any2api",
            "any2api-domain",
            "any2api-protocol",
            "any2api-provider",
            "any2api-runtime",
            "any2api-server",
            "any2api-storage",
            "any2api-transport",
            "any2api-updater",
        ],
    ),
];

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let metadata = MetadataCommand::new()
        .current_dir(workspace)
        .no_deps()
        .exec()?;
    let workspace_ids: BTreeSet<_> = metadata.workspace_members.iter().collect();
    let workspace_names: BTreeSet<_> = metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
        .map(|package| package.name.to_string())
        .collect();

    for (package, _) in DEPENDENCY_POLICY {
        if !workspace_names.contains(*package) {
            bail!("dependency policy contains unknown workspace package: {package}");
        }
    }

    for package in metadata
        .packages
        .iter()
        .filter(|package| workspace_ids.contains(&package.id))
    {
        check_package(package, &workspace_names)?;
    }

    Ok(())
}

fn check_package(package: &Package, workspace_names: &BTreeSet<String>) -> Result<()> {
    let Some((_, allowed)) = DEPENDENCY_POLICY
        .iter()
        .find(|(name, _)| *name == package.name.as_str())
    else {
        bail!(
            "workspace package is missing dependency policy: {}",
            package.name
        );
    };

    for dependency in package
        .dependencies
        .iter()
        .filter(|dependency| workspace_names.contains(dependency.name.as_str()))
    {
        if !allowed.contains(&dependency.name.as_str()) {
            bail!(
                "forbidden workspace dependency: {} -> {}",
                package.name,
                dependency.name
            );
        }
    }

    Ok(())
}
