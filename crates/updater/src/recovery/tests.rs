use std::fs;

use semver::Version;
use tempfile::tempdir;

use super::{StartupUpdateRecovery, UpdatePaths, commit_candidate, write_pending_state};

fn target() -> Version {
    Version::parse("1.2.3").expect("target version")
}

#[test]
fn committed_candidate_retains_previous_until_startup_confirmation() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    let staged = directory.path().join("any2api.new");
    fs::write(&executable, b"old").expect("old executable");
    fs::write(&staged, b"new").expect("new executable");

    commit_candidate(&staged, &executable, &target()).expect("commit candidate");

    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    assert_eq!(fs::read(&executable).expect("current executable"), b"new");
    assert_eq!(
        fs::read(&paths.previous).expect("previous executable"),
        b"old"
    );
    assert!(paths.pending.is_file());

    let mut recovery = StartupUpdateRecovery::discover(&executable, "1.2.3")
        .expect("discover update")
        .expect("pending update");
    assert!(recovery.target_matches("1.2.3"));
    recovery.confirm_startup();

    assert!(recovery.is_confirmed());
    assert!(!paths.previous.exists());
    assert!(!paths.pending.exists());
    assert_eq!(fs::read(executable).expect("confirmed executable"), b"new");
}

#[test]
fn startup_failure_atomically_restores_previous_executable() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    let staged = directory.path().join("any2api.new");
    fs::write(&executable, b"old").expect("old executable");
    fs::write(&staged, b"new").expect("new executable");
    commit_candidate(&staged, &executable, &target()).expect("commit candidate");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");

    StartupUpdateRecovery::discover(&executable, "1.2.3")
        .expect("discover update")
        .expect("pending update")
        .rollback()
        .expect("restore previous");

    assert_eq!(fs::read(&executable).expect("restored executable"), b"old");
    assert!(!paths.previous.exists());
    assert!(!paths.pending.exists());
}

#[test]
fn rollback_restores_previous_when_current_path_is_missing() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    let staged = directory.path().join("any2api.new");
    fs::write(&executable, b"old").expect("old executable");
    fs::write(&staged, b"new").expect("new executable");
    commit_candidate(&staged, &executable, &target()).expect("commit candidate");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    fs::remove_file(&executable).expect("remove broken current path");

    StartupUpdateRecovery::discover(&executable, "1.2.3")
        .expect("discover update")
        .expect("pending update")
        .rollback()
        .expect("restore missing executable");

    assert_eq!(fs::read(&executable).expect("restored executable"), b"old");
    assert!(!paths.previous.exists());
    assert!(!paths.pending.exists());
}

#[test]
fn interrupted_commit_with_same_inode_converges_without_removing_current() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    fs::write(&executable, b"old").expect("old executable");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    write_pending_state(&paths.pending, &target()).expect("pending marker");
    fs::hard_link(&executable, &paths.previous).expect("previous hard link");

    StartupUpdateRecovery::discover(&executable, "1.0.0")
        .expect("discover update")
        .expect("pending update")
        .rollback()
        .expect("converge interrupted commit");

    assert_eq!(fs::read(&executable).expect("current executable"), b"old");
    assert!(!paths.previous.exists());
    assert!(!paths.pending.exists());
}

#[test]
fn marker_without_previous_is_cleaned_as_an_incomplete_commit() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    fs::write(&executable, b"current").expect("current executable");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    write_pending_state(&paths.pending, &target()).expect("pending marker");

    let recovery =
        StartupUpdateRecovery::discover(&executable, "1.2.3").expect("inspect incomplete update");

    assert!(recovery.is_none());
    assert!(!paths.pending.exists());
    assert_eq!(
        fs::read(executable).expect("current executable"),
        b"current"
    );
}

#[test]
fn existing_previous_file_is_never_overwritten() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    let staged = directory.path().join("any2api.new");
    fs::write(&executable, b"old").expect("old executable");
    fs::write(&staged, b"new").expect("new executable");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    fs::write(&paths.previous, b"unowned").expect("existing previous");

    let error = commit_candidate(&staged, &executable, &target())
        .expect_err("existing previous must block commit");

    assert!(error.to_string().contains("preserve previous"));
    assert_eq!(fs::read(&executable).expect("current executable"), b"old");
    assert_eq!(
        fs::read(&paths.previous).expect("existing previous"),
        b"unowned"
    );
    assert!(!paths.pending.exists());
}

#[test]
fn existing_pending_marker_is_never_overwritten() {
    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api");
    let staged = directory.path().join("any2api.new");
    fs::write(&executable, b"old").expect("old executable");
    fs::write(&staged, b"new").expect("new executable");
    let paths = UpdatePaths::for_executable(&executable).expect("update paths");
    fs::write(&paths.pending, b"unowned").expect("existing pending marker");

    commit_candidate(&staged, &executable, &target())
        .expect_err("existing pending marker must block commit");

    assert_eq!(fs::read(&executable).expect("current executable"), b"old");
    assert_eq!(
        fs::read(&paths.pending).expect("existing marker"),
        b"unowned"
    );
    assert!(!paths.previous.exists());
}
