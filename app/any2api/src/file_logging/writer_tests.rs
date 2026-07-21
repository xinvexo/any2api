use std::{fs, sync::RwLock};

use any2api_domain::ConfigRevision;
use tempfile::tempdir;
use time::{Duration, OffsetDateTime};

use super::*;

const MIB: u64 = 1024 * 1024;

#[test]
fn rotates_by_size_and_utc_date() {
    let directory = tempdir().expect("temporary directory");
    let policy = Arc::new(RwLock::new(test_policy(7 * 86_400_000, MIB)));
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
    let policy = Arc::new(RwLock::new(test_policy(60_000, MIB)));
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
    let policy = Arc::new(RwLock::new(test_policy(86_400_000, MIB)));
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

fn test_policy(retention_ms: u64, max_total_size: u64) -> FileLogPolicy {
    FileLogPolicy {
        revision: ConfigRevision::INITIAL,
        retention_ms,
        max_total_size,
    }
}

fn managed_path(directory: &Path, sequence: u32) -> PathBuf {
    directory.join(format!("any2api-2026-07-21-{sequence:06}.jsonl"))
}
