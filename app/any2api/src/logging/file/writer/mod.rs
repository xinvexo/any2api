use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use any2api_storage::api::{ensure_private_directory, protect_private_file};
use time::OffsetDateTime;

use super::policy::FileLogPolicy;
use segments::{ActiveFile, date_key, open_segment};

mod segments;

pub(super) use segments::managed_files;

#[cfg(test)]
use segments::is_managed_file;

const MAX_SEGMENT_SIZE: u64 = 32 * 1024 * 1024;
const SEGMENTS_PER_CAPACITY: u64 = 8;
pub(super) struct RotatingFileWriter {
    directory: PathBuf,
    policy: Arc<RwLock<FileLogPolicy>>,
    active: Option<ActiveFile>,
    applied_revision: Option<any2api_domain::ConfigRevision>,
}

impl RotatingFileWriter {
    pub(super) fn new(directory: PathBuf, policy: Arc<RwLock<FileLogPolicy>>) -> io::Result<Self> {
        ensure_private_directory(&directory)?;
        for file in managed_files(&directory, None)? {
            protect_private_file(&file.path)?;
        }
        let mut writer = Self {
            directory,
            policy,
            active: None,
            applied_revision: None,
        };
        writer.maintain(OffsetDateTime::now_utc(), 0)?;
        Ok(writer)
    }

    fn write_at(&mut self, now: OffsetDateTime, bytes: &[u8]) -> io::Result<()> {
        let policy = *self.policy.read().expect("file log policy");
        let date = date_key(now);
        let target = segment_target(policy.max_total_size);
        let policy_changed = self.applied_revision != Some(policy.revision);
        let rotate = self.active.as_ref().is_some_and(|active| {
            active.date != date
                || (active.bytes > 0 && active.bytes.saturating_add(bytes.len() as u64) > target)
                || (policy_changed && active.bytes >= target)
        });

        if rotate {
            self.active.take();
        }
        if policy_changed || self.active.is_none() {
            self.maintain(now, target)?;
        }
        if self.active.is_none() {
            self.active = Some(open_segment(&self.directory, date)?);
        }
        self.applied_revision = Some(policy.revision);

        let active = self.active.as_mut().expect("active file opened");
        active.file.write_all(bytes)?;
        active.bytes = active.bytes.saturating_add(bytes.len() as u64);
        Ok(())
    }

    fn maintain(&mut self, now: OffsetDateTime, reserved_bytes: u64) -> io::Result<()> {
        let policy = *self.policy.read().expect("file log policy");
        let active_path = self.active.as_ref().map(|active| active.path.as_path());
        let mut files = managed_files(&self.directory, active_path)?;
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
}

impl Write for RotatingFileWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_at(OffsetDateTime::now_utc(), bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(active) = self.active.as_mut() {
            active.file.flush()?;
        }
        Ok(())
    }
}

fn segment_target(max_total_size: u64) -> u64 {
    (max_total_size / SEGMENTS_PER_CAPACITY).clamp(1, MAX_SEGMENT_SIZE)
}

#[cfg(test)]
mod tests;
