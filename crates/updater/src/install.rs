use std::{
    fs::File,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use reqwest::Client;
use semver::Version;
use tar::Archive;

use crate::{
    api::{UpdateError, UpdateErrorKind, UpdateTaskExecutor},
    github::{self, MAX_ARCHIVE_BYTES, Release},
    recovery, smoke, temporary,
};

const MAX_BINARY_BYTES: u64 = 256 * 1024 * 1024;

pub(crate) struct PreparedInstall {
    temporary: tempfile::TempDir,
    archive_path: PathBuf,
    staged_path: PathBuf,
    executable_path: PathBuf,
    target_version: Version,
}

impl PreparedInstall {
    pub(crate) fn commit(self) -> Result<(), UpdateError> {
        let Self {
            temporary,
            archive_path,
            staged_path,
            executable_path,
            target_version,
        } = self;
        let result = extract_and_replace(
            &archive_path,
            &staged_path,
            &executable_path,
            &target_version,
        );
        drop(temporary);
        result
    }
}

pub(crate) async fn prepare_from_release<Progress>(
    client: &Client,
    release: &Release,
    executable_path: &Path,
    tasks: &dyn UpdateTaskExecutor,
    progress: Progress,
) -> Result<PreparedInstall, UpdateError>
where
    Progress: FnMut(u64) + Send,
{
    let parent = executable_path
        .parent()
        .ok_or_else(|| install_failed("current executable has no parent directory"))?;
    let cleanup_target = executable_path.to_owned();
    tasks
        .run_blocking(Box::new(move || {
            let cleanup =
                temporary::cleanup_stale_for_executable(&cleanup_target).map_err(|error| {
                    install_failed(format!("failed to clean stale update directories: {error}"))
                })?;
            log_cleanup(cleanup, "before installation");
            Ok(())
        }))
        .await?;
    let temporary = tempfile::Builder::new()
        .prefix(temporary::DIRECTORY_PREFIX)
        .rand_bytes(temporary::DIRECTORY_RANDOM_LEN)
        .tempdir_in(parent)
        .map_err(|error| install_failed(format!("failed to create update directory: {error}")))?;
    let archive_path = temporary.path().join("release.tar.gz");
    let checksum = github::download_checksum(client, release).await?;
    let actual_digest = github::download_archive(client, release, &archive_path, progress).await?;
    verify_checksum(&checksum, &release.archive_name, &actual_digest)?;

    Ok(PreparedInstall {
        staged_path: temporary.path().join("any2api.new"),
        temporary,
        archive_path,
        executable_path: executable_path.to_owned(),
        target_version: release.version.clone(),
    })
}

fn log_cleanup(cleanup: temporary::CleanupReport, phase: &str) {
    if cleanup.removed > 0 {
        tracing::info!(removed = cleanup.removed, %phase, "removed stale update directories");
    }
    if cleanup.skipped > 0 {
        tracing::warn!(
            skipped = cleanup.skipped,
            %phase,
            "left update-like paths untouched because ownership, type, or contents were unsafe"
        );
    }
}

fn verify_checksum(
    checksum: &[u8],
    archive_name: &str,
    actual_digest: &str,
) -> Result<(), UpdateError> {
    let text = std::str::from_utf8(checksum)
        .map_err(|_| verification_failed("checksum file is not UTF-8"))?;
    let mut lines = text.lines();
    let first = lines
        .next()
        .ok_or_else(|| verification_failed("checksum file is empty"))?;
    if lines.any(|line| !line.trim().is_empty()) {
        return Err(verification_failed("checksum file contains extra records"));
    }
    let mut fields = first.split_whitespace();
    let expected_digest = fields
        .next()
        .ok_or_else(|| verification_failed("checksum digest is missing"))?;
    let expected_name = fields
        .next()
        .ok_or_else(|| verification_failed("checksum asset name is missing"))?;
    if fields.next().is_some()
        || expected_name != archive_name
        || expected_digest.len() != 64
        || !expected_digest
            .bytes()
            .all(|value| value.is_ascii_hexdigit())
    {
        return Err(verification_failed("checksum file has an invalid format"));
    }
    if !expected_digest.eq_ignore_ascii_case(actual_digest) {
        return Err(verification_failed(
            "release archive checksum does not match",
        ));
    }
    Ok(())
}

fn extract_and_replace(
    archive_path: &Path,
    staged_path: &Path,
    executable_path: &Path,
    target_version: &Version,
) -> Result<(), UpdateError> {
    let current_metadata = std::fs::symlink_metadata(executable_path).map_err(|error| {
        install_failed(format!("failed to inspect current executable: {error}"))
    })?;
    if !current_metadata.file_type().is_file() {
        return Err(install_failed("current executable is not a regular file"));
    }
    let archive_file = File::open(archive_path)
        .map_err(|error| verification_failed(format!("failed to open release archive: {error}")))?;
    if archive_file
        .metadata()
        .map_err(|error| {
            verification_failed(format!("failed to inspect release archive: {error}"))
        })?
        .len()
        > MAX_ARCHIVE_BYTES
    {
        return Err(verification_failed(
            "release archive exceeds the size limit",
        ));
    }
    let mut archive = Archive::new(GzDecoder::new(archive_file));
    let mut entries = archive
        .entries()
        .map_err(|error| verification_failed(format!("invalid release archive: {error}")))?;
    let mut entry = entries
        .next()
        .ok_or_else(|| verification_failed("release archive is empty"))?
        .map_err(|error| verification_failed(format!("invalid release archive entry: {error}")))?;
    let path = entry
        .path()
        .map_err(|error| verification_failed(format!("invalid release archive path: {error}")))?;
    if path.as_ref() != Path::new("any2api")
        || !entry.header().entry_type().is_file()
        || entry.size() == 0
        || entry.size() > MAX_BINARY_BYTES
    {
        return Err(verification_failed(
            "release archive must contain only the root any2api binary",
        ));
    }
    entry.unpack(staged_path).map_err(|error| {
        verification_failed(format!("failed to extract release binary: {error}"))
    })?;
    drop(entry);
    if entries.next().is_some() {
        return Err(verification_failed(
            "release archive contains extra entries",
        ));
    }

    set_executable_permissions(staged_path, &current_metadata)?;
    File::open(staged_path)
        .and_then(|file| file.sync_all())
        .map_err(|error| install_failed(format!("failed to sync release binary: {error}")))?;
    smoke::verify_staged_version(staged_path, target_version)?;
    recovery::commit_candidate(staged_path, executable_path, target_version)
        .map_err(|error| install_failed(error.to_string()))
}

#[cfg(unix)]
fn set_executable_permissions(
    staged_path: &Path,
    current_metadata: &std::fs::Metadata,
) -> Result<(), UpdateError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = current_metadata.permissions().mode() & 0o777;
    std::fs::set_permissions(staged_path, std::fs::Permissions::from_mode(mode))
        .map_err(|error| install_failed(format!("failed to set executable permissions: {error}")))
}

#[cfg(not(unix))]
fn set_executable_permissions(
    _staged_path: &Path,
    _current_metadata: &std::fs::Metadata,
) -> Result<(), UpdateError> {
    Err(install_failed("in-place updates require a Unix host"))
}

fn verification_failed(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::VerificationFailed, message)
}

fn install_failed(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::InstallFailed, message)
}

#[cfg(test)]
pub(crate) fn verify_checksum_for_test(
    checksum: &[u8],
    archive_name: &str,
    actual_digest: &str,
) -> Result<(), UpdateError> {
    verify_checksum(checksum, archive_name, actual_digest)
}

#[cfg(test)]
pub(crate) fn extract_and_replace_for_test(
    archive_path: &Path,
    staged_path: &Path,
    executable_path: &Path,
    target_version: &Version,
) -> Result<(), UpdateError> {
    extract_and_replace(archive_path, staged_path, executable_path, target_version)
}
