use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, SystemTime},
};

use time::{Date, Month, OffsetDateTime};

use super::policy::FileLogPolicy;

const MAX_SEGMENT_SIZE: u64 = 32 * 1024 * 1024;
const SEGMENTS_PER_CAPACITY: u64 = 8;
const FILE_PREFIX: &str = "any2api-";
const FILE_SUFFIX: &str = ".jsonl";

pub(super) struct RotatingFileWriter {
    directory: PathBuf,
    policy: Arc<RwLock<FileLogPolicy>>,
    active: Option<ActiveFile>,
    applied_revision: Option<any2api_domain::ConfigRevision>,
}

struct ActiveFile {
    path: PathBuf,
    date: String,
    bytes: u64,
    file: File,
}

pub(super) struct ManagedFile {
    pub(super) path: PathBuf,
    modified: SystemTime,
    bytes: u64,
}

impl RotatingFileWriter {
    pub(super) fn new(directory: PathBuf, policy: Arc<RwLock<FileLogPolicy>>) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
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

fn open_segment(directory: &Path, date: String) -> io::Result<ActiveFile> {
    for sequence in 0_u32.. {
        let path = directory.join(format!("{FILE_PREFIX}{date}-{sequence:06}{FILE_SUFFIX}"));
        match OpenOptions::new().create_new(true).append(true).open(&path) {
            Ok(file) => {
                return Ok(ActiveFile {
                    path,
                    date,
                    bytes: 0,
                    file,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    unreachable!("u32 segment sequence is exhaustive")
}

pub(super) fn managed_files(
    directory: &Path,
    active: Option<&Path>,
) -> io::Result<Vec<ManagedFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if active.is_some_and(|active| active == path) || !is_managed_file(&path) {
            continue;
        }
        if entry.file_type()?.is_file() {
            let metadata = entry.metadata()?;
            files.push(ManagedFile {
                path,
                modified: metadata.modified()?,
                bytes: metadata.len(),
            });
        }
    }
    Ok(files)
}

fn is_managed_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let Some(body) = name
        .strip_prefix(FILE_PREFIX)
        .and_then(|name| name.strip_suffix(FILE_SUFFIX))
    else {
        return false;
    };
    let Some((date, sequence)) = body.rsplit_once('-') else {
        return false;
    };
    valid_date_key(date)
        && !sequence.is_empty()
        && sequence.bytes().all(|byte| byte.is_ascii_digit())
}

fn valid_date_key(value: &str) -> bool {
    if value.len() != 10
        || value.as_bytes()[4] != b'-'
        || value.as_bytes()[7] != b'-'
        || !value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return false;
    }
    let Ok(year) = value[0..4].parse::<i32>() else {
        return false;
    };
    let Some(month) = value[5..7]
        .parse::<u8>()
        .ok()
        .and_then(|value| Month::try_from(value).ok())
    else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u8>() else {
        return false;
    };
    Date::from_calendar_date(year, month, day).is_ok()
}

fn date_key(now: OffsetDateTime) -> String {
    let date = now.date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

fn segment_target(max_total_size: u64) -> u64 {
    (max_total_size / SEGMENTS_PER_CAPACITY).clamp(1, MAX_SEGMENT_SIZE)
}

#[cfg(test)]
#[path = "writer_tests.rs"]
mod tests;
