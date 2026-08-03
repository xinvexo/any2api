use std::{
    fs,
    path::{Path, PathBuf},
    sync::RwLock,
};

use any2api_domain::ConfigRevision;
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

use super::*;

const MIB: u64 = 1024 * 1024;

#[test]
fn rotates_by_size_and_utc_date() {
    let directory = tempdir().expect("temporary directory");
    let policy = Arc::new(RwLock::new(test_policy(7 * 86_400, MIB)));
    let mut writer =
        RotatingFileWriter::new(directory.path().to_path_buf(), policy).expect("rotating writer");
    let now = OffsetDateTime::now_utc();
    let block = vec![b'x'; 100 * 1024];

    writer.write_at(now, &block).expect("first segment");
    writer.write_at(now, &block).expect("size rotation");
    writer
        .write_at(now + Duration::DAY, b"next day\n")
        .expect("date rotation");

    assert_eq!(managed_files(directory.path(), None).unwrap().len(), 3);
}

#[test]
fn removes_expired_managed_files_but_keeps_active_and_unmanaged_files() {
    let directory = tempdir().expect("temporary directory");
    let policy = Arc::new(RwLock::new(test_policy(60, MIB)));
    let mut writer =
        RotatingFileWriter::new(directory.path().to_path_buf(), policy).expect("rotating writer");
    let now = OffsetDateTime::now_utc();
    let expired = directory.path().join("any2api-2026-01-01-000001.jsonl");
    let unmanaged = directory.path().join("notes.jsonl");
    fs::write(&expired, b"expired").expect("expired file");
    fs::write(&unmanaged, b"keep").expect("unmanaged file");
    writer.write_at(now, b"active\n").expect("active file");
    let active = writer.active.as_ref().expect("active segment").path.clone();

    writer
        .maintain(now + Duration::minutes(2), 0)
        .expect("retention cleanup");

    assert!(!expired.exists());
    assert!(unmanaged.exists());
    assert!(active.exists());
}

#[test]
fn removes_oldest_closed_segments_until_capacity_fits() {
    let directory = tempdir().expect("temporary directory");
    let policy = Arc::new(RwLock::new(test_policy(86_400, MIB)));
    let mut writer =
        RotatingFileWriter::new(directory.path().to_path_buf(), policy).expect("rotating writer");
    let first = managed_path(directory.path(), 1);
    let second = managed_path(directory.path(), 2);
    let third = managed_path(directory.path(), 3);
    let unmanaged = directory.path().join("other.log");
    for path in [&first, &second, &third] {
        fs::write(path, vec![b'x'; 400 * 1024]).expect("managed segment");
    }
    fs::write(&unmanaged, vec![b'x'; 2 * MIB as usize]).expect("unmanaged file");

    writer
        .maintain(OffsetDateTime::now_utc(), segment_target(MIB))
        .expect("capacity cleanup");

    assert!(!first.exists());
    assert!(second.exists());
    assert!(third.exists());
    assert!(unmanaged.exists());
}

#[test]
fn managed_file_names_require_a_real_calendar_date() {
    assert!(is_managed_file(Path::new(
        "any2api-2024-02-29-000001.jsonl"
    )));
    assert!(!is_managed_file(Path::new(
        "any2api-2026-02-29-000001.jsonl"
    )));
    assert!(!is_managed_file(Path::new(
        "any2api-2026-13-01-000001.jsonl"
    )));
    assert!(!is_managed_file(Path::new("notes.jsonl")));
}

#[cfg(unix)]
#[test]
fn log_directory_and_segments_are_private() {
    use std::os::unix::fs::PermissionsExt;

    let root = tempdir().expect("temporary directory");
    let directory = root.path().join("logs");
    fs::create_dir(&directory).expect("log directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
        .expect("directory permissions");
    let now = OffsetDateTime::now_utc();
    let existing = directory.join(format!("any2api-{}-000001.jsonl", date_key(now)));
    fs::write(&existing, b"existing").expect("existing log");
    fs::set_permissions(&existing, fs::Permissions::from_mode(0o644))
        .expect("existing permissions");

    let policy = Arc::new(RwLock::new(test_policy(86_400, MIB)));
    let mut writer = RotatingFileWriter::new(directory.clone(), policy).expect("writer");
    writer.write_at(now, b"new\n").expect("new segment");

    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&existing), 0o600);
    assert_eq!(
        mode(&writer.active.as_ref().expect("active segment").path),
        0o600
    );
}

#[cfg(unix)]
#[test]
fn deleted_active_log_directory_is_recreated_with_a_visible_new_segment() {
    let root = tempdir().expect("temporary directory");
    let directory = root.path().join("logs");
    let policy = Arc::new(RwLock::new(test_policy(86_400, MIB)));
    let mut writer = RotatingFileWriter::new(directory.clone(), policy).expect("writer");
    let now = OffsetDateTime::now_utc();
    writer
        .write_at(now, b"before deletion\n")
        .expect("first write");

    fs::remove_dir_all(&directory).expect("remove active log directory");
    writer.directory.force_next_check();
    writer
        .write_at(now, b"after recreation\n")
        .expect("self-healed write");

    let files = managed_files(&directory, None).expect("recreated log files");
    assert_eq!(files.len(), 1);
    assert_eq!(
        fs::read_to_string(&files[0].path).expect("recreated log contents"),
        "after recreation\n"
    );
    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&files[0].path), 0o600);
}

#[cfg(unix)]
#[test]
fn writer_resumes_after_an_unsafe_directory_blocker_is_removed() {
    let root = tempdir().expect("temporary directory");
    let directory = root.path().join("logs");
    let policy = Arc::new(RwLock::new(test_policy(86_400, MIB)));
    let mut writer = RotatingFileWriter::new(directory.clone(), policy).expect("writer");
    let now = OffsetDateTime::now_utc();
    writer
        .write_at(now, b"before blocker\n")
        .expect("first write");

    fs::remove_dir_all(&directory).expect("remove log directory");
    fs::write(&directory, b"unsafe blocker").expect("directory blocker");
    writer.directory.force_next_check();
    assert!(writer.write_at(now, b"while blocked\n").is_err());

    fs::remove_file(&directory).expect("remove directory blocker");
    writer
        .write_at(now, b"after recovery\n")
        .expect("write after external recovery");

    let files = managed_files(&directory, None).expect("recovered log files");
    assert_eq!(files.len(), 1);
    assert_eq!(
        fs::read_to_string(&files[0].path).expect("recovered log contents"),
        "after recovery\n"
    );
}

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn test_policy(retention_secs: u64, max_total_size: u64) -> FileLogPolicy {
    FileLogPolicy {
        revision: ConfigRevision::INITIAL,
        retention_secs,
        max_total_size,
    }
}

fn managed_path(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!("any2api-2026-07-21-{sequence:06}.jsonl"))
}
