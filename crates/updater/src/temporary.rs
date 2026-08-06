use std::{io, path::Path};

#[cfg(unix)]
use std::{ffi::OsStr, fs, io::ErrorKind, path::PathBuf};

pub(crate) const DIRECTORY_PREFIX: &str = ".any2api-update-";
pub(crate) const DIRECTORY_RANDOM_LEN: usize = 6;

#[cfg(unix)]
const ARCHIVE_NAME: &str = "release.tar.gz";
#[cfg(unix)]
const STAGED_BINARY_NAME: &str = "any2api.new";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CleanupReport {
    pub(crate) removed: usize,
    pub(crate) skipped: usize,
}

pub(crate) fn cleanup_stale_for_executable(executable: &Path) -> io::Result<CleanupReport> {
    let Some(parent) = executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Ok(CleanupReport::default());
    };
    cleanup_stale(parent)
}

#[cfg(unix)]
fn cleanup_stale(parent: &Path) -> io::Result<CleanupReport> {
    cleanup_stale_owned_by(parent, rustix::process::geteuid().as_raw())
}

#[cfg(not(unix))]
fn cleanup_stale(_parent: &Path) -> io::Result<CleanupReport> {
    Ok(CleanupReport::default())
}

#[cfg(unix)]
fn cleanup_stale_owned_by(parent: &Path, expected_uid: u32) -> io::Result<CleanupReport> {
    use std::os::unix::fs::MetadataExt;

    let mut report = CleanupReport::default();
    for entry in fs::read_dir(parent).map_err(|error| path_error("scan", parent, error))? {
        let entry = entry.map_err(|error| path_error("read an entry from", parent, error))?;
        if !is_update_directory_name(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(path_error("inspect", &path, error)),
        };
        if !metadata.file_type().is_dir() || metadata.uid() != expected_uid {
            report.skipped += 1;
            continue;
        }

        let Some(files) = removable_files(&path, expected_uid)? else {
            report.skipped += 1;
            continue;
        };
        for file in files {
            match fs::remove_file(&file) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(path_error("remove", &file, error)),
            }
        }
        match fs::remove_dir(&path) {
            Ok(()) => report.removed += 1,
            Err(error) if error.kind() == ErrorKind::NotFound => report.removed += 1,
            Err(error) if error.kind() == ErrorKind::DirectoryNotEmpty => report.skipped += 1,
            Err(error) => return Err(path_error("remove", &path, error)),
        }
    }
    Ok(report)
}

#[cfg(unix)]
fn removable_files(path: &Path, expected_uid: u32) -> io::Result<Option<Vec<PathBuf>>> {
    use std::os::unix::fs::MetadataExt;

    let mut files = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| path_error("scan", path, error))? {
        let entry = entry.map_err(|error| path_error("read an entry from", path, error))?;
        if !is_known_file_name(&entry.file_name()) {
            return Ok(None);
        }
        let child = entry.path();
        let metadata = match fs::symlink_metadata(&child) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(path_error("inspect", &child, error)),
        };
        if !metadata.file_type().is_file() || metadata.uid() != expected_uid {
            return Ok(None);
        }
        files.push(child);
    }
    Ok(Some(files))
}

#[cfg(unix)]
fn is_update_directory_name(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix(DIRECTORY_PREFIX))
        .is_some_and(|suffix| {
            suffix.len() == DIRECTORY_RANDOM_LEN
                && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

#[cfg(unix)]
fn is_known_file_name(name: &OsStr) -> bool {
    name == ARCHIVE_NAME || name == STAGED_BINARY_NAME
}

#[cfg(unix)]
fn path_error(operation: &str, path: &Path, error: io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("failed to {operation} {}: {error}", path.display()),
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::{fs, os::unix::fs::symlink};

    use tempfile::tempdir;

    use super::{CleanupReport, cleanup_stale_owned_by};

    #[test]
    fn cleanup_removes_only_exact_owned_workspaces_with_known_regular_files() {
        let parent = tempdir().expect("temporary directory");
        let removable = parent.path().join(".any2api-update-Ab12Z9");
        fs::create_dir(&removable).expect("removable directory");
        fs::write(removable.join("release.tar.gz"), b"archive").expect("archive");
        fs::write(removable.join("any2api.new"), b"candidate").expect("candidate");
        let empty = parent.path().join(".any2api-update-Ef34Y8");
        fs::create_dir(&empty).expect("empty directory");

        let regular = parent.path().join(".any2api-update-Fi56X7");
        fs::write(&regular, b"keep").expect("regular lookalike");
        let target = parent.path().join("symlink-target");
        fs::create_dir(&target).expect("symlink target");
        fs::write(target.join("keep"), b"keep").expect("target contents");
        let linked = parent.path().join(".any2api-update-Gh78W6");
        symlink(&target, &linked).expect("directory symlink");
        let unknown = parent.path().join(".any2api-update-Ij90V5");
        fs::create_dir(&unknown).expect("unknown directory");
        fs::write(unknown.join("user-data"), b"keep").expect("unknown contents");
        let child_link = parent.path().join(".any2api-update-Kl12U4");
        fs::create_dir(&child_link).expect("child-link directory");
        symlink(target.join("keep"), child_link.join("any2api.new")).expect("child symlink");
        let near_name = parent.path().join(".any2api-update-important");
        fs::create_dir(&near_name).expect("near-name directory");

        let uid = std::os::unix::fs::MetadataExt::uid(
            &fs::symlink_metadata(parent.path()).expect("parent metadata"),
        );
        let report = cleanup_stale_owned_by(parent.path(), uid).expect("cleanup");

        assert_eq!(
            report,
            CleanupReport {
                removed: 2,
                skipped: 4,
            }
        );
        assert!(!removable.exists());
        assert!(!empty.exists());
        assert_eq!(fs::read(&regular).expect("regular preserved"), b"keep");
        assert!(linked.is_symlink());
        assert_eq!(
            fs::read(target.join("keep")).expect("target preserved"),
            b"keep"
        );
        assert!(unknown.join("user-data").is_file());
        assert!(child_link.join("any2api.new").is_symlink());
        assert!(near_name.is_dir());
    }

    #[test]
    fn cleanup_preserves_a_matching_directory_owned_by_another_uid() {
        let parent = tempdir().expect("temporary directory");
        let candidate = parent.path().join(".any2api-update-Mn34T2");
        fs::create_dir(&candidate).expect("candidate directory");
        fs::write(candidate.join("release.tar.gz"), b"archive").expect("archive");
        let uid = std::os::unix::fs::MetadataExt::uid(
            &fs::symlink_metadata(&candidate).expect("candidate metadata"),
        );

        let report = cleanup_stale_owned_by(parent.path(), uid ^ 1).expect("cleanup");

        assert_eq!(
            report,
            CleanupReport {
                removed: 0,
                skipped: 1,
            }
        );
        assert!(candidate.join("release.tar.gz").is_file());
    }
}
