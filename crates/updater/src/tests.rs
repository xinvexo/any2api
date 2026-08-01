use std::{
    fs,
    io::Write,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use flate2::{Compression, write::GzEncoder};
use serde_json::json;
use tar::{Builder, Header};
use tempfile::tempdir;

use crate::{
    api::{
        ApplicationUpdateService, RestartRequester, UpdateErrorKind, UpdateStatus, UpdateTask,
        UpdateTaskExecutor,
    },
    github::{MAX_ARCHIVE_BYTES, parse_release_for_test},
    install::{extract_and_replace_for_test, verify_checksum_for_test},
    service::GitHubReleaseUpdater,
};

struct CapturingTaskExecutor {
    accepting: AtomicBool,
    spawn_accepting: AtomicBool,
    tasks: Mutex<Vec<UpdateTask>>,
}

impl CapturingTaskExecutor {
    fn new(accepting: bool, spawn_accepting: bool) -> Self {
        Self {
            accepting: AtomicBool::new(accepting),
            spawn_accepting: AtomicBool::new(spawn_accepting),
            tasks: Mutex::new(Vec::new()),
        }
    }

    fn take_task(&self) -> UpdateTask {
        self.tasks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop()
            .expect("captured update task")
    }

    fn task_count(&self) -> usize {
        self.tasks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

impl UpdateTaskExecutor for CapturingTaskExecutor {
    fn accepts_new_tasks(&self) -> bool {
        self.accepting.load(Ordering::Acquire)
    }

    fn try_spawn(&self, task: UpdateTask) -> bool {
        if !self.spawn_accepting.load(Ordering::Acquire) {
            return false;
        }
        self.tasks
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(task);
        true
    }
}

#[derive(Default)]
struct CountingRestartRequester(AtomicUsize);

impl RestartRequester for CountingRestartRequester {
    fn request_restart(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

#[test]
fn compiled_build_version_uses_the_release_override_or_dev_default() {
    let version = semver::Version::parse(crate::BUILD_VERSION).expect("build version is SemVer");
    if let Some(expected) = option_env!("ANY2API_BUILD_VERSION") {
        assert_eq!(crate::BUILD_VERSION, expected);
        assert!(version.pre.is_empty());
        assert!(version.build.is_empty());
    } else {
        assert_eq!(crate::BUILD_VERSION, "0.0.0-dev");
    }
}

#[tokio::test]
async fn accepted_install_task_continues_after_the_start_call_returns() {
    let tasks = Arc::new(CapturingTaskExecutor::new(true, true));
    let completed = Arc::new(AtomicBool::new(false));
    let task_completed = Arc::clone(&completed);
    let updater = test_updater(tasks.clone(), Arc::new(CountingRestartRequester::default()));

    assert_eq!(
        updater
            .start_task_for_test(async move {
                task_completed.store(true, Ordering::Release);
            })
            .expect("accepted update task"),
        UpdateStatus::Checking
    );
    assert!(!completed.load(Ordering::Acquire));

    tasks.take_task().await;

    assert!(completed.load(Ordering::Acquire));
}

#[test]
fn lifecycle_rejection_rolls_back_task_admission() {
    let tasks = Arc::new(CapturingTaskExecutor::new(true, false));
    let updater = test_updater(tasks.clone(), Arc::new(CountingRestartRequester::default()));

    let error = updater
        .start_task_for_test(std::future::ready(()))
        .expect_err("task registration must fail");

    assert_eq!(error.kind(), UpdateErrorKind::ShuttingDown);
    assert_eq!(updater.install_status(), UpdateStatus::Idle);
    assert_eq!(tasks.task_count(), 0);
}

#[test]
fn non_running_lifecycle_rejects_before_task_admission() {
    let tasks = Arc::new(CapturingTaskExecutor::new(false, false));
    let updater = test_updater(tasks.clone(), Arc::new(CountingRestartRequester::default()));

    let error = updater
        .start_install()
        .expect_err("shutting down lifecycle must reject");

    assert_eq!(error.kind(), UpdateErrorKind::ShuttingDown);
    assert_eq!(updater.install_status(), UpdateStatus::Idle);
    assert_eq!(tasks.task_count(), 0);
}

#[test]
fn completed_install_requests_restart_exactly_once() {
    let tasks = Arc::new(CapturingTaskExecutor::new(true, true));
    let restart = Arc::new(CountingRestartRequester::default());
    let updater = test_updater(tasks, restart.clone());

    updater.complete_install_for_test("1.2.3");

    assert_eq!(restart.0.load(Ordering::Acquire), 1);
    assert_eq!(
        updater.install_status(),
        UpdateStatus::Restarting {
            target_version: "1.2.3".to_owned(),
        }
    );
}

#[test]
fn release_metadata_requires_exact_stable_assets() {
    let archive = "any2api-v1.2.3-linux-amd64.tar.gz";
    let document = json!({
        "tag_name": "v1.2.3",
        "draft": false,
        "prerelease": false,
        "published_at": "2026-07-29T00:00:00Z",
        "assets": [
            { "name": archive, "size": 1234 },
            { "name": format!("{archive}.sha256"), "size": 100 }
        ]
    });
    let release = parse_release_for_test(&serde_json::to_vec(&document).expect("metadata"))
        .expect("valid release");
    assert_eq!(release.version.to_string(), "1.2.3");
    assert_eq!(release.archive_name, archive);

    let mut oversized = document.clone();
    oversized["assets"][0]["size"] = json!(MAX_ARCHIVE_BYTES + 1);
    let error = parse_release_for_test(&serde_json::to_vec(&oversized).expect("large metadata"))
        .expect_err("oversized archive must fail");
    assert_eq!(error.kind(), UpdateErrorKind::InvalidRelease);

    let mut missing_checksum = document;
    missing_checksum["assets"] = json!([{ "name": archive, "size": 1234 }]);
    let error =
        parse_release_for_test(&serde_json::to_vec(&missing_checksum).expect("invalid metadata"))
            .expect_err("checksum is required");
    assert_eq!(error.kind(), UpdateErrorKind::InvalidRelease);
}

#[test]
fn checksum_must_name_and_match_the_archive() {
    let name = "any2api-v1.2.3-linux-amd64.tar.gz";
    let digest = "a".repeat(64);
    verify_checksum_for_test(format!("{digest}  {name}\n").as_bytes(), name, &digest)
        .expect("valid checksum");
    let error = verify_checksum_for_test(
        format!("{}  {name}\n", "b".repeat(64)).as_bytes(),
        name,
        &digest,
    )
    .expect_err("mismatch must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
}

#[test]
fn verified_archive_atomically_replaces_the_binary() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    fs::write(&executable, b"old binary").expect("old binary");
    let archive = directory.path().join("release.tar.gz");
    write_archive(&archive, &[("any2api", b"new binary")]);

    extract_and_replace_for_test(&archive, &directory.path().join("staged"), &executable)
        .expect("replace binary");
    assert_eq!(fs::read(executable).expect("new binary"), b"new binary");
}

#[cfg(unix)]
#[test]
fn replacement_preserves_restrictive_executable_permissions() {
    use std::os::unix::fs::PermissionsExt;

    for mode in [0o700, 0o555] {
        let directory = tempdir().expect("temporary directory");
        let executable = directory.path().join("any2api");
        fs::write(&executable, b"old binary").expect("old binary");
        fs::set_permissions(&executable, fs::Permissions::from_mode(mode))
            .expect("old permissions");
        let archive = directory.path().join("release.tar.gz");
        write_archive(&archive, &[("any2api", b"new binary")]);

        extract_and_replace_for_test(&archive, &directory.path().join("staged"), &executable)
            .expect("replace binary");
        let actual = fs::metadata(executable)
            .expect("new binary")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(actual, mode);
    }
}

#[test]
fn archive_with_extra_entries_is_rejected_before_replacement() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    fs::write(&executable, b"old binary").expect("old binary");
    let archive = directory.path().join("release.tar.gz");
    write_archive(&archive, &[("any2api", b"new binary"), ("extra", b"bad")]);

    let error =
        extract_and_replace_for_test(&archive, &directory.path().join("staged"), &executable)
            .expect_err("extra archive member must fail");
    assert_eq!(error.kind(), UpdateErrorKind::VerificationFailed);
    assert_eq!(fs::read(executable).expect("old binary"), b"old binary");
}

fn write_archive(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("archive file");
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = Builder::new(encoder);
    for (name, contents) in entries {
        let mut header = Header::new_gnu();
        header.set_size(u64::try_from(contents.len()).expect("content size"));
        header.set_mode(0o755);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *contents)
            .expect("archive entry");
    }
    let encoder = builder.into_inner().expect("tar finish");
    encoder
        .finish()
        .expect("gzip finish")
        .flush()
        .expect("flush");
}

fn test_updater(
    tasks: Arc<dyn UpdateTaskExecutor>,
    restart: Arc<dyn RestartRequester>,
) -> GitHubReleaseUpdater {
    GitHubReleaseUpdater::new(std::path::PathBuf::new(), true, restart, tasks)
        .expect("test updater")
}
