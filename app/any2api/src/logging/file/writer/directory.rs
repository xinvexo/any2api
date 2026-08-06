use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use any2api_storage::api::{ensure_private_directory, protect_private_file};

use super::segments::{ActiveFile, managed_files};

const CHECK_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct LogDirectory {
    path: PathBuf,
    next_check: Instant,
}

impl LogDirectory {
    pub(super) fn open(path: PathBuf) -> io::Result<Self> {
        ensure_private_directory(&path)?;
        protect_managed_segments(&path)?;
        Ok(Self {
            path,
            next_check: Instant::now(),
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn check_if_due(&mut self, active: Option<&ActiveFile>) -> io::Result<()> {
        let now = Instant::now();
        if now < self.next_check {
            return Ok(());
        }
        self.next_check = next_check_after(now);

        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "file log directory has an unsafe file type: {}",
                    self.path.display()
                ),
            ));
        }
        ensure_private_directory(&self.path)?;
        if let Some(active) = active
            && !active_path_matches(active)?
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "active file log segment is no longer linked at {}",
                    active.path.display()
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn restore(&mut self) -> io::Result<()> {
        ensure_private_directory(&self.path)?;
        protect_managed_segments(&self.path)?;
        self.next_check = next_check_after(Instant::now());
        Ok(())
    }

    #[cfg(all(test, unix))]
    pub(super) fn force_next_check(&mut self) {
        self.next_check = Instant::now();
    }
}

fn protect_managed_segments(directory: &Path) -> io::Result<()> {
    for file in managed_files(directory, None)? {
        protect_private_file(&file.path)?;
    }
    Ok(())
}

fn next_check_after(now: Instant) -> Instant {
    now.checked_add(CHECK_INTERVAL).unwrap_or(now)
}

#[cfg(unix)]
fn active_path_matches(active: &ActiveFile) -> io::Result<bool> {
    use std::os::unix::fs::MetadataExt;

    let opened = active.file.metadata()?;
    let linked = match fs::symlink_metadata(&active.path) {
        Ok(metadata) if metadata.file_type().is_file() => metadata,
        Ok(_) => return Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(opened.dev() == linked.dev() && opened.ino() == linked.ino())
}

#[cfg(not(unix))]
fn active_path_matches(active: &ActiveFile) -> io::Result<bool> {
    Ok(fs::symlink_metadata(&active.path).is_ok_and(|metadata| metadata.file_type().is_file()))
}
