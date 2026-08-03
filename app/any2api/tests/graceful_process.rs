#![cfg(unix)]

#[path = "graceful_process/captured_child.rs"]
mod captured_child;

use std::{
    fs,
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use captured_child::CapturedChild;
use tempfile::tempdir;

#[test]
fn version_mode_has_no_startup_side_effects() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("must-not-exist");
    let output = Command::new(env!("CARGO_BIN_EXE_any2api"))
        .arg("--version")
        .env("ANY2API_BIND", "not-a-socket-address")
        .env("ANY2API_DATA_DIR", &data)
        .output()
        .expect("run version command");

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        format!("any2api {}\n", any2api_updater::api::APPLICATION_VERSION).as_bytes()
    );
    assert!(output.stderr.is_empty());
    assert!(!data.exists());
}

#[test]
fn unknown_arguments_fail_before_creating_the_data_directory() {
    let directory = tempdir().expect("temporary directory");
    let data = directory.path().join("must-not-exist");
    let output = Command::new(env!("CARGO_BIN_EXE_any2api"))
        .arg("--unknown")
        .env("ANY2API_DATA_DIR", &data)
        .output()
        .expect("run invalid command");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage: any2api [--version]"));
    assert!(!data.exists());
}

#[test]
fn sqlite_startup_failure_is_reported_by_bootstrap_tracing() {
    let directory = tempdir().expect("temporary data directory");
    fs::write(
        directory.path().join("any2api.sqlite3"),
        b"not a sqlite database",
    )
    .expect("corrupt database fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_any2api"))
        .env("ANY2API_BIND", "127.0.0.1:0")
        .env("ANY2API_DATA_DIR", directory.path())
        .env("RUST_LOG", "any2api=info")
        .output()
        .expect("run any2api with corrupt database");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("any2api startup failed"), "{stderr}");
    assert!(stderr.contains("phase=\"application\""), "{stderr}");
    assert!(
        stderr.contains("failed to initialize sqlite storage"),
        "{stderr}"
    );
    assert!(!directory.path().join("logs").exists());
}

#[test]
fn child_output_larger_than_pipe_capacity_is_drained_while_running() {
    let script = r#"
        i=0
        while [ "$i" -lt 20000 ]; do
            printf 'debug-line-%05d\n' "$i"
            i=$((i + 1))
        done
        printf 'stderr-diagnostic\n' >&2
    "#;
    let child = Command::new("/bin/sh")
        .args(["-c", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start high-output child");
    let mut child = CapturedChild::new(child);

    let status = wait_until_exited(&mut child);
    let output = child.output_after_exit();

    assert!(status.success(), "high-output child failed: {output}");
    assert!(
        output.len() > 256 * 1024,
        "only {} output bytes",
        output.len()
    );
    assert!(output.contains("debug-line-19999"), "{output}");
    assert!(output.contains("stderr-diagnostic"), "{output}");
}

#[test]
fn startup_failure_before_listener_confirmation_restores_previous_binary() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api-copy");
    fs::copy(env!("CARGO_BIN_EXE_any2api"), &executable).expect("copy candidate binary");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
        .expect("candidate permissions");
    let previous = directory.path().join("any2api-copy.previous");
    let previous_contents = b"#!/bin/sh\nprintf recovered > \"$ANY2API_RECOVERY_SENTINEL\"\n";
    fs::write(&previous, previous_contents).expect("previous executable");
    fs::set_permissions(&previous, fs::Permissions::from_mode(0o755))
        .expect("previous permissions");
    let pending = directory.path().join("any2api-copy.update-pending");
    fs::write(
        &pending,
        format!(
            "{{\"format\":\"any2api-self-update-v1\",\"target_version\":\"{}\"}}\n",
            any2api_updater::api::APPLICATION_VERSION
        ),
    )
    .expect("pending marker");
    let sentinel = directory.path().join("recovered");
    let occupied = TcpListener::bind("127.0.0.1:0").expect("occupy listener");
    let output = Command::new(&executable)
        .env("ANY2API_INTERNAL_UPDATE_RESTART", "1")
        .env("ANY2API_BIND", occupied.local_addr().unwrap().to_string())
        .env("ANY2API_DATA_DIR", directory.path().join("data"))
        .env("ANY2API_ADMIN_PASSWORD", "application-test-password")
        .env("ANY2API_RECOVERY_SENTINEL", &sentinel)
        .output()
        .expect("run updated candidate");

    assert!(
        output.status.success(),
        "recovered process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(&sentinel).expect("recovery sentinel"),
        b"recovered"
    );
    assert_eq!(
        fs::read(&executable).expect("restored executable"),
        previous_contents
    );
    assert!(!previous.exists());
    assert!(!pending.exists());
    assert!(String::from_utf8_lossy(&output.stderr).contains("restored previous executable"));
}

#[test]
fn successful_listener_start_confirms_update_and_removes_recovery_files() {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempdir().expect("temporary directory");
    let executable = directory.path().join("any2api-copy");
    fs::copy(env!("CARGO_BIN_EXE_any2api"), &executable).expect("copy candidate binary");
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))
        .expect("candidate permissions");
    let previous = directory.path().join("any2api-copy.previous");
    fs::write(&previous, b"previous").expect("previous executable");
    let pending = directory.path().join("any2api-copy.update-pending");
    fs::write(
        &pending,
        format!(
            "{{\"format\":\"any2api-self-update-v1\",\"target_version\":\"{}\"}}\n",
            any2api_updater::api::APPLICATION_VERSION
        ),
    )
    .expect("pending marker");
    let address = available_address();
    let child = Command::new(&executable)
        .env("ANY2API_INTERNAL_UPDATE_RESTART", "1")
        .env("ANY2API_BIND", address.to_string())
        .env("ANY2API_DATA_DIR", directory.path().join("data"))
        .env("ANY2API_ADMIN_PASSWORD", "application-test-password")
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start updated candidate");
    let mut child = CapturedChild::new(child);

    wait_until_listening(&mut child, address);
    wait_until_update_confirmed(&mut child, &previous, &pending);
    let signal = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success(), "kill command failed: {signal}");
    let status = wait_until_exited(&mut child);
    let output = child.output_after_exit();
    assert!(status.success(), "any2api exited with {status}\n{output}");
    assert!(output.contains("any2api shutdown complete"), "{output}");
}

#[test]
fn sigterm_completes_the_full_application_shutdown() {
    let directory = tempdir().expect("temporary data directory");
    let address = available_address();
    let child = Command::new(env!("CARGO_BIN_EXE_any2api"))
        .env("ANY2API_BIND", address.to_string())
        .env("ANY2API_DATA_DIR", directory.path())
        .env("ANY2API_ADMIN_PASSWORD", "application-test-password")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start any2api");
    let mut child = CapturedChild::new(child);

    wait_until_listening(&mut child, address);
    let signal = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success(), "kill command failed: {signal}");

    let status = wait_until_exited(&mut child);
    let output = child.output_after_exit();
    assert!(status.success(), "any2api exited with {status}\n{output}");
}

#[test]
fn empty_data_dir_uses_the_protected_default_directory() {
    let directory = tempdir().expect("temporary working directory");
    let address = available_address();
    let child = Command::new(env!("CARGO_BIN_EXE_any2api"))
        .current_dir(directory.path())
        .env("ANY2API_BIND", address.to_string())
        .env("ANY2API_DATA_DIR", "")
        .env("ANY2API_ADMIN_PASSWORD", "application-test-password")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start any2api");
    let mut child = CapturedChild::new(child);

    wait_until_listening(&mut child, address);
    let data = directory.path().join("data");
    assert_eq!(mode(&data), 0o700);
    assert_eq!(mode(&data.join("any2api.sqlite3")), 0o600);
    assert_eq!(mode(&data.join("any2api.instance.lock")), 0o600);
    assert!(!directory.path().join("any2api.sqlite3").exists());
    assert!(!directory.path().join("any2api.instance.lock").exists());

    let signal = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success(), "kill command failed: {signal}");
    let status = wait_until_exited(&mut child);
    let output = child.output_after_exit();
    assert!(status.success(), "any2api exited with {status}\n{output}");
}

fn mode(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    fs::metadata(path)
        .unwrap_or_else(|error| panic!("read {} metadata: {error}", path.display()))
        .permissions()
        .mode()
        & 0o777
}

fn available_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local port");
    listener.local_addr().expect("local address")
}

fn wait_until_listening(child: &mut CapturedChild, address: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            return;
        }
        if let Some(status) = child.try_wait().expect("inspect child") {
            let output = child.output_after_exit();
            panic!("any2api exited before listening with {status}\n{output}");
        }
        assert!(Instant::now() < deadline, "any2api did not start in time");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_until_exited(child: &mut CapturedChild) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().expect("inspect child") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "any2api did not stop after SIGTERM"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_until_update_confirmed(
    child: &mut CapturedChild,
    previous: &std::path::Path,
    pending: &std::path::Path,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while previous.exists() || pending.exists() {
        if let Some(status) = child.try_wait().expect("inspect child") {
            let output = child.output_after_exit();
            panic!("any2api exited before update confirmation with {status}\n{output}");
        }
        assert!(
            Instant::now() < deadline,
            "any2api did not confirm startup in time"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
