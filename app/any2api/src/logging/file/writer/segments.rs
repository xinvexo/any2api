use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use time::{Date, Month, OffsetDateTime};

const FILE_PREFIX: &str = "any2api-";
const FILE_SUFFIX: &str = ".jsonl";

pub(super) struct ActiveFile {
    pub(super) path: PathBuf,
    pub(super) date: String,
    pub(super) bytes: u64,
    pub(super) file: File,
}

pub(in crate::logging::file) struct ManagedFile {
    pub(in crate::logging::file) path: PathBuf,
    pub(in crate::logging::file) modified: SystemTime,
    pub(in crate::logging::file) bytes: u64,
}

pub(super) fn open_segment(directory: &Path, date: String) -> io::Result<ActiveFile> {
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

pub(in crate::logging::file) fn managed_files(
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

pub(super) fn is_managed_file(path: &Path) -> bool {
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

pub(super) fn date_key(now: OffsetDateTime) -> String {
    let date = now.date();
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}
