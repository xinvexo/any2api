use std::{
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use tempfile::tempdir;

use crate::{
    api::{
        ApplicationUpdateService, RestartRequester, UpdateCommitTask, UpdateError, UpdateErrorKind,
        UpdateStatus, UpdateTask, UpdateTaskExecutor,
    },
    service::GitHubReleaseUpdater,
};

const TARGET_VERSION: &str = "1.2.3";

struct CapturingTaskExecutor {
    accepting: AtomicBool,
    spawn_accepting: AtomicBool,
    tasks: Mutex<Vec<UpdateTask>>,
    commits: Mutex<Vec<UpdateCommitTask>>,
}

impl CapturingTaskExecutor {
    fn new(accepting: bool, spawn_accepting: bool) -> Self {
        Self {
            accepting: AtomicBool::new(accepting),
            spawn_accepting: AtomicBool::new(spawn_accepting),
            tasks: Mutex::new(Vec::new()),
            commits: Mutex::new(Vec::new()),
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

    fn take_commit(&self) -> UpdateCommitTask {
        self.commits
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop()
            .expect("captured update commit")
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

    fn spawn_blocking_commit(&self, task: UpdateCommitTask) {
        self.commits
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(task);
    }
}

#[derive(Default)]
struct CountingRestartRequester(AtomicUsize);

impl RestartRequester for CountingRestartRequester {
    fn request_restart(&self) {
        self.0.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(unix)]
#[test]
fn updater_initialization_cleans_only_recognized_stale_workspaces() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    write_executable(&executable, b"current binary", 0o755);
    let stale = directory.path().join(".any2api-update-Qr56S1");
    fs::create_dir(&stale).expect("stale update directory");
    fs::write(stale.join("release.tar.gz"), b"partial archive").expect("stale archive");
    let lookalike = directory.path().join(".any2api-update-user-files");
    fs::create_dir(&lookalike).expect("lookalike directory");
    fs::write(lookalike.join("keep"), b"keep").expect("lookalike contents");

    GitHubReleaseUpdater::new(
        executable,
        true,
        Arc::new(CountingRestartRequester::default()),
        Arc::new(CapturingTaskExecutor::new(true, true)),
    )
    .expect("updater");

    assert!(!stale.exists());
    assert_eq!(
        fs::read(lookalike.join("keep")).expect("lookalike preserved"),
        b"keep"
    );
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

    updater.complete_install_for_test(TARGET_VERSION);

    assert_eq!(restart.0.load(Ordering::Acquire), 1);
    assert_eq!(
        updater.install_status(),
        UpdateStatus::Restarting {
            target_version: TARGET_VERSION.to_owned(),
        }
    );
}

#[test]
fn successful_blocking_commit_owns_the_restart_transition() {
    let tasks = Arc::new(CapturingTaskExecutor::new(true, true));
    let restart = Arc::new(CountingRestartRequester::default());
    let updater = test_updater(tasks.clone(), restart.clone());

    updater.schedule_commit_for_test(TARGET_VERSION, || Ok(()));
    assert_eq!(restart.0.load(Ordering::Acquire), 0);
    (tasks.take_commit())();

    assert_eq!(restart.0.load(Ordering::Acquire), 1);
    assert_eq!(
        updater.install_status(),
        UpdateStatus::Restarting {
            target_version: TARGET_VERSION.to_owned(),
        }
    );
}

#[test]
fn failed_blocking_commit_owns_the_failure_transition() {
    let tasks = Arc::new(CapturingTaskExecutor::new(true, true));
    let restart = Arc::new(CountingRestartRequester::default());
    let updater = test_updater(tasks.clone(), restart.clone());

    updater.schedule_commit_for_test(TARGET_VERSION, || {
        Err(UpdateError::new(
            UpdateErrorKind::InstallFailed,
            "injected commit failure",
        ))
    });
    (tasks.take_commit())();

    assert_eq!(restart.0.load(Ordering::Acquire), 0);
    assert_eq!(
        updater.install_status(),
        UpdateStatus::Failed {
            target_version: Some(TARGET_VERSION.to_owned()),
            kind: UpdateErrorKind::InstallFailed,
        }
    );
}

fn write_executable(path: &std::path::Path, contents: &[u8], mode: u32) {
    fs::write(path, contents).expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set executable mode");
    }
    #[cfg(not(unix))]
    let _ = mode;
}

fn test_updater(
    tasks: Arc<dyn UpdateTaskExecutor>,
    restart: Arc<dyn RestartRequester>,
) -> GitHubReleaseUpdater {
    GitHubReleaseUpdater::new(std::path::PathBuf::new(), true, restart, tasks)
        .expect("test updater")
}
