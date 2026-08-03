use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use time::OffsetDateTime;

use super::policy::FileLogPolicy;
use diagnostics::IoDiagnostics;
use directory::LogDirectory;
use retention::maintain;
use segments::{ActiveFile, date_key, open_segment};

mod diagnostics;
mod directory;
mod retention;
mod segments;

#[cfg(test)]
pub(super) use segments::managed_files;

#[cfg(test)]
use segments::is_managed_file;

const MAX_SEGMENT_SIZE: u64 = 32 * 1024 * 1024;
const SEGMENTS_PER_CAPACITY: u64 = 8;

pub(super) struct RotatingFileWriter {
    directory: LogDirectory,
    policy: Arc<RwLock<FileLogPolicy>>,
    active: Option<ActiveFile>,
    applied_revision: Option<any2api_domain::ConfigRevision>,
    diagnostics: IoDiagnostics,
}

impl RotatingFileWriter {
    pub(super) fn new(directory: PathBuf, policy: Arc<RwLock<FileLogPolicy>>) -> io::Result<Self> {
        let directory = LogDirectory::open(directory)?;
        let mut writer = Self {
            directory,
            policy,
            active: None,
            applied_revision: None,
            diagnostics: IoDiagnostics::default(),
        };
        writer.maintain(OffsetDateTime::now_utc(), 0)?;
        Ok(writer)
    }

    fn write_at(&mut self, now: OffsetDateTime, bytes: &[u8]) -> io::Result<()> {
        let result = self.write_event(now, bytes);
        match &result {
            Ok(()) if !bytes.is_empty() => self.diagnostics.report_recovery(),
            Ok(()) => {}
            Err(error) => self.diagnostics.report_failure(error),
        }
        result
    }

    fn write_event(&mut self, now: OffsetDateTime, bytes: &[u8]) -> io::Result<()> {
        self.prepare_with_recovery(now, bytes.len() as u64)?;
        let result = {
            let active = self.active.as_mut().expect("active file opened");
            let result = active.file.write_all(bytes);
            if result.is_ok() {
                active.bytes = active.bytes.saturating_add(bytes.len() as u64);
            } else if let Ok(metadata) = active.file.metadata() {
                active.bytes = metadata.len();
            }
            result
        };
        if result.is_err() {
            self.active.take();
            self.applied_revision = None;
        }
        result
    }

    fn prepare_with_recovery(
        &mut self,
        now: OffsetDateTime,
        incoming_bytes: u64,
    ) -> io::Result<()> {
        if let Err(error) = self.prepare(now, incoming_bytes) {
            self.diagnostics.report_failure(&error);
            self.restore_directory()?;
            self.prepare(now, incoming_bytes)?;
        }
        Ok(())
    }

    fn prepare(&mut self, now: OffsetDateTime, incoming_bytes: u64) -> io::Result<()> {
        self.directory.check_if_due(self.active.as_ref())?;
        let policy = *self.policy.read().expect("file log policy");
        let date = date_key(now);
        let target = segment_target(policy.max_total_size);
        let policy_changed = self.applied_revision != Some(policy.revision);
        let rotate = self.active.as_ref().is_some_and(|active| {
            active.date != date
                || (active.bytes > 0 && active.bytes.saturating_add(incoming_bytes) > target)
                || (policy_changed && active.bytes >= target)
        });

        if rotate {
            self.active.take();
        }
        if policy_changed || self.active.is_none() {
            self.maintain(now, target)?;
        }
        if self.active.is_none() {
            self.active = Some(open_segment(self.directory.path(), date)?);
        }
        self.applied_revision = Some(policy.revision);
        Ok(())
    }

    fn restore_directory(&mut self) -> io::Result<()> {
        self.active.take();
        self.applied_revision = None;
        self.directory.restore()
    }

    fn maintain(&mut self, now: OffsetDateTime, reserved_bytes: u64) -> io::Result<()> {
        let policy = *self.policy.read().expect("file log policy");
        let active_path = self.active.as_ref().map(|active| active.path.as_path());
        maintain(
            self.directory.path(),
            active_path,
            now,
            reserved_bytes,
            policy,
        )
    }
}

impl Write for RotatingFileWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.write_at(OffsetDateTime::now_utc(), bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let result = self
            .active
            .as_mut()
            .map_or(Ok(()), |active| active.file.flush());
        if let Err(error) = &result {
            self.diagnostics.report_failure(error);
            self.active.take();
            self.applied_revision = None;
        }
        result
    }
}

fn segment_target(max_total_size: u64) -> u64 {
    (max_total_size / SEGMENTS_PER_CAPACITY).clamp(1, MAX_SEGMENT_SIZE)
}

#[cfg(test)]
mod tests;
