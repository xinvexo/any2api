use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const STATE_FORMAT: &str = "any2api-self-update-v1";
const MAX_STATE_BYTES: u64 = 1024;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct UpdateRecoveryError {
    message: String,
}

impl UpdateRecoveryError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PendingState {
    format: String,
    target_version: String,
}

#[derive(Clone)]
struct UpdatePaths {
    executable: PathBuf,
    previous: PathBuf,
    pending: PathBuf,
    parent: PathBuf,
}

impl UpdatePaths {
    fn for_executable(executable: &Path) -> Result<Self, UpdateRecoveryError> {
        let parent = executable
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| {
                UpdateRecoveryError::new("current executable has no parent directory")
            })?;
        let file_name = executable
            .file_name()
            .ok_or_else(|| UpdateRecoveryError::new("current executable has no file name"))?;
        Ok(Self {
            executable: executable.to_owned(),
            previous: sibling(parent, file_name, ".previous"),
            pending: sibling(parent, file_name, ".update-pending"),
            parent: parent.to_owned(),
        })
    }

    fn sync_parent(&self) -> Result<(), UpdateRecoveryError> {
        File::open(&self.parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                UpdateRecoveryError::new(format!(
                    "failed to sync executable directory {}: {error}",
                    self.parent.display()
                ))
            })
    }
}

fn sibling(parent: &Path, file_name: &std::ffi::OsStr, suffix: &str) -> PathBuf {
    let mut name = OsString::from(file_name);
    name.push(suffix);
    parent.join(name)
}

pub struct StartupUpdateRecovery {
    paths: UpdatePaths,
    target_version: String,
    confirmed: bool,
}

impl StartupUpdateRecovery {
    pub fn discover(
        executable: &Path,
        running_version: &str,
    ) -> Result<Option<Self>, UpdateRecoveryError> {
        let paths = UpdatePaths::for_executable(executable)?;
        let Some(state) = read_pending_state(&paths.pending)? else {
            return Ok(None);
        };
        if fs::symlink_metadata(&paths.previous)
            .is_err_and(|error| error.kind() == ErrorKind::NotFound)
        {
            remove_regular_file(&paths.pending, "pending update marker")?;
            paths.sync_parent()?;
            return Ok(None);
        }
        require_regular_file(&paths.previous, "previous executable")?;
        let recovery = Self {
            paths,
            target_version: state.target_version,
            confirmed: false,
        };
        if recovery.target_version != running_version {
            tracing::warn!(
                expected_version = %recovery.target_version,
                %running_version,
                "pending update version does not match the running binary"
            );
        }
        Ok(Some(recovery))
    }

    #[must_use]
    pub fn target_matches(&self, running_version: &str) -> bool {
        self.target_version == running_version
    }

    #[must_use]
    pub const fn is_confirmed(&self) -> bool {
        self.confirmed
    }

    pub fn confirm_startup(&mut self) {
        self.confirmed = true;
        if let Err(error) = cleanup_confirmed(&self.paths) {
            tracing::warn!(%error, "failed to clean confirmed application update state");
        }
    }

    pub fn rollback(self) -> Result<(), UpdateRecoveryError> {
        if self.confirmed {
            return Err(UpdateRecoveryError::new(
                "confirmed application update cannot be rolled back automatically",
            ));
        }
        restore_previous(&self.paths)
    }
}

pub fn recover_pending_update(
    executable: &Path,
    running_version: &str,
) -> Result<bool, UpdateRecoveryError> {
    let Some(recovery) = StartupUpdateRecovery::discover(executable, running_version)? else {
        return Ok(false);
    };
    recovery.rollback()?;
    Ok(true)
}

pub(crate) fn commit_candidate(
    staged: &Path,
    executable: &Path,
    target_version: &Version,
) -> Result<(), UpdateRecoveryError> {
    let paths = UpdatePaths::for_executable(executable)?;
    write_pending_state(&paths.pending, target_version)?;
    if let Err(error) = fs::hard_link(&paths.executable, &paths.previous) {
        let cleanup = remove_regular_file(&paths.pending, "pending update marker");
        return Err(with_cleanup(
            UpdateRecoveryError::new(format!("failed to preserve previous executable: {error}")),
            cleanup,
        ));
    }
    if let Err(error) = paths.sync_parent() {
        return Err(with_cleanup(error, cleanup_uncommitted(&paths)));
    }
    if let Err(error) = fs::rename(staged, &paths.executable) {
        let error =
            UpdateRecoveryError::new(format!("failed to replace current executable: {error}"));
        return Err(with_cleanup(error, cleanup_uncommitted(&paths)));
    }
    if let Err(error) = paths.sync_parent() {
        return Err(with_cleanup(error, restore_previous(&paths)));
    }
    Ok(())
}

fn write_pending_state(path: &Path, target: &Version) -> Result<(), UpdateRecoveryError> {
    let state = PendingState {
        format: STATE_FORMAT.to_owned(),
        target_version: target.to_string(),
    };
    let mut bytes = serde_json::to_vec(&state).map_err(|error| {
        UpdateRecoveryError::new(format!("failed to encode update state: {error}"))
    })?;
    bytes.push(b'\n');
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        UpdateRecoveryError::new(format!("failed to create pending update marker: {error}"))
    })?;
    if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
        let primary = UpdateRecoveryError::new(format!("failed to sync update state: {error}"));
        drop(file);
        return Err(with_cleanup(
            primary,
            remove_regular_file(path, "pending update marker"),
        ));
    }
    Ok(())
}

fn read_pending_state(path: &Path) -> Result<Option<PendingState>, UpdateRecoveryError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(UpdateRecoveryError::new(format!(
                "failed to inspect pending update marker: {error}"
            )));
        }
    };
    if !metadata.file_type().is_file() || metadata.len() > MAX_STATE_BYTES {
        return Err(UpdateRecoveryError::new(
            "pending update marker is not a small regular file",
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|error| {
            UpdateRecoveryError::new(format!("failed to read update state: {error}"))
        })?;
    let state: PendingState = serde_json::from_slice(&bytes).map_err(|error| {
        UpdateRecoveryError::new(format!("invalid pending update marker: {error}"))
    })?;
    if state.format != STATE_FORMAT || Version::parse(&state.target_version).is_err() {
        return Err(UpdateRecoveryError::new(
            "pending update marker has an unsupported format or version",
        ));
    }
    Ok(Some(state))
}

fn cleanup_uncommitted(paths: &UpdatePaths) -> Result<(), UpdateRecoveryError> {
    remove_regular_file(&paths.previous, "previous executable")?;
    remove_regular_file(&paths.pending, "pending update marker")?;
    paths.sync_parent()
}

fn cleanup_confirmed(paths: &UpdatePaths) -> Result<(), UpdateRecoveryError> {
    remove_regular_file_if_present(&paths.previous, "previous executable")?;
    remove_regular_file_if_present(&paths.pending, "pending update marker")?;
    paths.sync_parent()
}

fn restore_previous(paths: &UpdatePaths) -> Result<(), UpdateRecoveryError> {
    require_regular_file(&paths.previous, "previous executable")?;
    if same_file(&paths.executable, &paths.previous)? {
        remove_regular_file(&paths.previous, "previous executable")?;
    } else {
        fs::rename(&paths.previous, &paths.executable).map_err(|error| {
            UpdateRecoveryError::new(format!("failed to restore previous executable: {error}"))
        })?;
    }
    remove_regular_file_if_present(&paths.pending, "pending update marker")?;
    paths.sync_parent()
}

fn require_regular_file(path: &Path, label: &str) -> Result<(), UpdateRecoveryError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| UpdateRecoveryError::new(format!("failed to inspect {label}: {error}")))?;
    if !metadata.file_type().is_file() {
        return Err(UpdateRecoveryError::new(format!(
            "{label} is not a regular file"
        )));
    }
    Ok(())
}

fn remove_regular_file(path: &Path, label: &str) -> Result<(), UpdateRecoveryError> {
    require_regular_file(path, label)?;
    fs::remove_file(path)
        .map_err(|error| UpdateRecoveryError::new(format!("failed to remove {label}: {error}")))
}

fn remove_regular_file_if_present(path: &Path, label: &str) -> Result<(), UpdateRecoveryError> {
    match fs::symlink_metadata(path) {
        Ok(_) => remove_regular_file(path, label),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UpdateRecoveryError::new(format!(
            "failed to inspect {label}: {error}"
        ))),
    }
}

#[cfg(unix)]
fn same_file(left: &Path, right: &Path) -> Result<bool, UpdateRecoveryError> {
    use std::os::unix::fs::MetadataExt;

    let left = match fs::symlink_metadata(left) {
        Ok(metadata) if metadata.file_type().is_file() => metadata,
        Ok(_) => {
            return Err(UpdateRecoveryError::new(
                "current executable is not a regular file",
            ));
        }
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(UpdateRecoveryError::new(format!(
                "failed to inspect executable: {error}"
            )));
        }
    };
    let right = fs::symlink_metadata(right).map_err(|error| {
        UpdateRecoveryError::new(format!("failed to inspect previous executable: {error}"))
    })?;
    Ok(left.dev() == right.dev() && left.ino() == right.ino())
}

#[cfg(not(unix))]
fn same_file(_left: &Path, _right: &Path) -> Result<bool, UpdateRecoveryError> {
    Ok(false)
}

fn with_cleanup(
    primary: UpdateRecoveryError,
    cleanup: Result<(), UpdateRecoveryError>,
) -> UpdateRecoveryError {
    match cleanup {
        Ok(()) => primary,
        Err(cleanup) => {
            UpdateRecoveryError::new(format!("{primary}; cleanup also failed: {cleanup}"))
        }
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
