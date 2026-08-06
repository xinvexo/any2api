use std::{
    io::{Read, Seek},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use semver::Version;

use crate::api::{UpdateError, UpdateErrorKind};

const VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const VERSION_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_VERSION_OUTPUT_BYTES: u64 = 256;

pub(crate) fn verify_staged_version(
    executable: &Path,
    target: &Version,
) -> Result<(), UpdateError> {
    verify_staged_version_with_timeout(executable, target, VERSION_TIMEOUT)
}

fn verify_staged_version_with_timeout(
    executable: &Path,
    target: &Version,
    timeout: Duration,
) -> Result<(), UpdateError> {
    let parent = executable
        .parent()
        .ok_or_else(|| verification_failed("staged executable has no parent directory"))?;
    let mut output = tempfile::tempfile_in(parent).map_err(|error| {
        verification_failed(format!("failed to create version smoke output: {error}"))
    })?;
    let stdout = output.try_clone().map_err(|error| {
        verification_failed(format!("failed to prepare version smoke output: {error}"))
    })?;
    let child = Command::new(executable)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            verification_failed(format!("failed to execute staged release binary: {error}"))
        })?;
    let status = wait_for_exit(child, &output, timeout)?;
    if !status.success() {
        return Err(verification_failed(format!(
            "staged release --version exited with {status}"
        )));
    }
    let length = output
        .metadata()
        .map_err(|error| verification_failed(format!("failed to inspect version output: {error}")))?
        .len();
    if length > MAX_VERSION_OUTPUT_BYTES {
        return Err(verification_failed(
            "staged release --version output exceeds the size limit",
        ));
    }
    output
        .rewind()
        .map_err(|error| verification_failed(format!("failed to read version output: {error}")))?;
    let mut actual = Vec::with_capacity(length as usize);
    output
        .read_to_end(&mut actual)
        .map_err(|error| verification_failed(format!("failed to read version output: {error}")))?;
    let expected = format!("any2api {target}\n");
    if actual != expected.as_bytes() {
        return Err(verification_failed(
            "staged release --version does not match the target release",
        ));
    }
    Ok(())
}

fn wait_for_exit(
    child: Child,
    output: &std::fs::File,
    timeout: Duration,
) -> Result<std::process::ExitStatus, UpdateError> {
    let mut child = ChildGuard(Some(child));
    let deadline = Instant::now() + timeout;
    loop {
        let output_length = output
            .metadata()
            .map_err(|error| {
                verification_failed(format!("failed to inspect version output: {error}"))
            })?
            .len();
        if output_length > MAX_VERSION_OUTPUT_BYTES {
            let _ = child.child_mut().kill();
            let _ = child.child_mut().wait();
            child.0.take();
            return Err(verification_failed(
                "staged release --version output exceeds the size limit",
            ));
        }
        match child.child_mut().try_wait() {
            Ok(Some(status)) => {
                child.0.take();
                return Ok(status);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(VERSION_POLL_INTERVAL),
            Ok(None) => {
                let _ = child.child_mut().kill();
                let _ = child.child_mut().wait();
                child.0.take();
                return Err(verification_failed("staged release --version timed out"));
            }
            Err(error) => {
                return Err(verification_failed(format!(
                    "failed to wait for staged release --version: {error}"
                )));
            }
        }
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn child_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("smoke child is present")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn verification_failed(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::VerificationFailed, message)
}

#[cfg(all(test, unix))]
pub(crate) fn verify_staged_version_for_test(
    executable: &Path,
    target: &Version,
    timeout: Duration,
) -> Result<(), UpdateError> {
    verify_staged_version_with_timeout(executable, target, timeout)
}
