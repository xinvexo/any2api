use std::{
    fs, io,
    path::Path,
    time::{Duration, SystemTime},
};

use time::OffsetDateTime;

use super::{FileLogPolicy, segments::managed_files};

pub(super) fn maintain(
    directory: &Path,
    active_path: Option<&Path>,
    now: OffsetDateTime,
    reserved_bytes: u64,
    policy: FileLogPolicy,
) -> io::Result<()> {
    let mut files = managed_files(directory, active_path)?;
    files.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });

    let cutoff = SystemTime::from(now)
        .checked_sub(Duration::from_secs(policy.retention_secs))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut retained = Vec::with_capacity(files.len());
    for file in files {
        if file.modified < cutoff {
            fs::remove_file(file.path)?;
        } else {
            retained.push(file);
        }
    }

    let mut total = reserved_bytes.saturating_add(
        retained
            .iter()
            .fold(0_u64, |sum, file| sum.saturating_add(file.bytes)),
    );
    for file in retained {
        if total <= policy.max_total_size {
            break;
        }
        fs::remove_file(file.path)?;
        total = total.saturating_sub(file.bytes);
    }
    Ok(())
}
