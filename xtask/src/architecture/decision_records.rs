use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

const INDEX_FILE: &str = "docs/adr/README.md";
const TEMPLATE_FILE: &str = "0000-template.md";

pub(crate) fn check(workspace: &Path) -> Result<()> {
    let adr_dir = workspace.join("docs/adr");
    let records = decision_records(&adr_dir)?;
    check_index(workspace, &records.current)?;
    check_markdown_references(workspace, records.all_ids)
}

struct DecisionRecords {
    all_ids: BTreeSet<u16>,
    current: BTreeMap<u16, PathBuf>,
}

fn decision_records(adr_dir: &Path) -> Result<DecisionRecords> {
    let mut all_ids = BTreeSet::new();
    let mut current = BTreeMap::new();
    let mut files = markdown_files(adr_dir)?;
    files.sort();

    for path in files {
        let name = file_name(&path)?.to_owned();
        if name == "README.md" || name == TEMPLATE_FILE {
            continue;
        }

        let id = name
            .get(..4)
            .filter(|_| name.as_bytes().get(4) == Some(&b'-'))
            .context("ADR filename must start with four digits and a dash")?
            .parse::<u16>()
            .with_context(|| format!("invalid ADR number in {name}"))?;
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let expected_heading = format!("# ADR-{id:04}");
        if !raw
            .lines()
            .next()
            .is_some_and(|line| line.starts_with(&expected_heading))
        {
            bail!("ADR heading must start with `{expected_heading}`: {name}");
        }
        if !all_ids.insert(id) {
            bail!("duplicate ADR-{id:04}: {name}");
        }
        if is_current_accepted(&raw, &name)? {
            current.insert(id, path);
        }
    }

    Ok(DecisionRecords { all_ids, current })
}

fn is_current_accepted(raw: &str, file_name: &str) -> Result<bool> {
    let status = raw
        .lines()
        .take(16)
        .map(|line| line.trim_start_matches(['>', '-', ' ']))
        .find_map(|line| {
            line.strip_prefix("状态：")
                .or_else(|| line.strip_prefix("状态:"))
                .or_else(|| line.strip_prefix("Status:"))
        })
        .map(str::trim)
        .with_context(|| format!("ADR is missing status metadata: {file_name}"))?;

    Ok(status.starts_with("Accepted"))
}

fn check_index(workspace: &Path, records: &BTreeMap<u16, PathBuf>) -> Result<()> {
    let index_path = workspace.join(INDEX_FILE);
    let raw =
        fs::read_to_string(&index_path).with_context(|| format!("failed to read {INDEX_FILE}"))?;
    let inventory = raw
        .split_once("## 完整当前清单")
        .context("ADR index is missing its complete current inventory")?
        .1;
    let mut indexed = BTreeMap::new();

    for line in inventory.lines().filter(|line| line.starts_with("| [")) {
        let label_end = line.find("](").context("invalid ADR index row")?;
        let id = line[3..label_end]
            .parse::<u16>()
            .context("invalid ADR index number")?;
        let target_start = label_end + 2;
        let target_end = line[target_start..]
            .find(')')
            .map(|offset| target_start + offset)
            .context("invalid ADR index link")?;
        let target = &line[target_start..target_end];
        if indexed.insert(id, target).is_some() {
            bail!("ADR-{id:04} appears more than once in the complete index");
        }
    }

    if indexed.len() != records.len() {
        bail!(
            "ADR index contains {} records but docs/adr contains {} current records",
            indexed.len(),
            records.len()
        );
    }

    for (id, path) in records {
        let expected = file_name(path)?;
        let actual = indexed
            .get(id)
            .with_context(|| format!("ADR-{id:04} is missing from {INDEX_FILE}"))?;
        if *actual != expected {
            bail!("ADR-{id:04} index target must be `{expected}`, found `{actual}`");
        }
    }
    Ok(())
}

fn check_markdown_references(workspace: &Path, ids: BTreeSet<u16>) -> Result<()> {
    let mut files = markdown_files(&workspace.join("docs"))?;
    files.push(workspace.join("ARCHITECTURE.md"));

    for path in files {
        if file_name(&path)? == TEMPLATE_FILE {
            continue;
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for id in referenced_adr_ids(&raw) {
            if !ids.contains(&id) {
                bail!("{} references missing ADR-{id:04}", path.display());
            }
        }
        check_local_markdown_links(workspace, &path, &raw)?;
        check_explicit_adr_paths(workspace, &path, &raw)?;
    }
    Ok(())
}

fn referenced_adr_ids(raw: &str) -> impl Iterator<Item = u16> + '_ {
    raw.match_indices("ADR-").filter_map(|(start, _)| {
        raw.get(start + 4..start + 8)
            .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))?
            .parse()
            .ok()
    })
}

fn check_local_markdown_links(workspace: &Path, source: &Path, raw: &str) -> Result<()> {
    for (start, _) in raw.match_indices("](") {
        let target = &raw[start + 2..];
        let Some(end) = target.find(')') else {
            continue;
        };
        let target = target[..end].trim_matches(['<', '>']);
        let target = target.split('#').next().unwrap_or_default();
        if target.is_empty()
            || target.contains("://")
            || target.starts_with("mailto:")
            || !target.ends_with(".md")
        {
            continue;
        }
        let resolved = source.parent().unwrap_or(workspace).join(target);
        if !resolved.is_file() {
            bail!("{} links to missing `{target}`", source.display());
        }
    }
    Ok(())
}

fn check_explicit_adr_paths(workspace: &Path, source: &Path, raw: &str) -> Result<()> {
    for (start, _) in raw.match_indices("docs/adr/") {
        let target = &raw[start..];
        let Some(end) = target.find(".md") else {
            continue;
        };
        let target = &target[..end + 3];
        let Some(relative) = target.strip_prefix("docs/adr/") else {
            continue;
        };
        if relative
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.')))
        {
            continue;
        }
        if !workspace.join(target).is_file() {
            bail!("{} references missing `{target}`", source.display());
        }
    }
    Ok(())
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read directory {}", root.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(markdown_files(&path)?);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
    Ok(files)
}

fn file_name(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .context("documentation filename must be valid UTF-8")
}
