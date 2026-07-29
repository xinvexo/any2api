#![cfg(unix)]

use std::{
    io::Read,
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use tempfile::tempdir;

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
    let mut child = ChildGuard(child);

    wait_until_listening(&mut child.0, address);
    let signal = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .expect("send SIGTERM");
    assert!(signal.success(), "kill command failed: {signal}");

    let status = wait_until_exited(&mut child.0);
    let output = captured_output(&mut child.0);
    assert!(status.success(), "any2api exited with {status}\n{output}");
}

fn available_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve local port");
    listener.local_addr().expect("local address")
}

fn wait_until_listening(child: &mut Child, address: SocketAddr) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok() {
            return;
        }
        if let Some(status) = child.try_wait().expect("inspect child") {
            let output = captured_output(child);
            panic!("any2api exited before listening with {status}\n{output}");
        }
        assert!(Instant::now() < deadline, "any2api did not start in time");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_until_exited(child: &mut Child) -> std::process::ExitStatus {
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

fn captured_output(child: &mut Child) -> String {
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_string(&mut output).expect("read stdout");
    }
    if let Some(mut stderr) = child.stderr.take() {
        stderr.read_to_string(&mut output).expect("read stderr");
    }
    output
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
