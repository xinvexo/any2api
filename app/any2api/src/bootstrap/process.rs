use std::{ffi::OsString, path::Path, process::Command};

use anyhow::Context;

use super::{application, environment::StartupSettings, instance_lock::InstanceLock};
use crate::shutdown;

pub fn run() -> anyhow::Result<()> {
    let settings = StartupSettings::from_env()?;
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    let executable_path = std::env::current_exe().unwrap_or_else(|error| {
        eprintln!(
            "any2api could not resolve its executable path; self-update is disabled: {error}"
        );
        std::path::PathBuf::new()
    });
    let data_directory = settings
        .database_path
        .parent()
        .context("database path must have a data directory")?;
    let instance_lock = InstanceLock::acquire(data_directory)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to initialize Tokio runtime")?;

    let started = runtime.block_on(application::run(settings, executable_path.clone()));
    let (result, timeout, fatal, restart) = match started {
        Ok(outcome) => {
            let timeout = outcome.runtime_shutdown_timeout();
            let fatal = outcome.is_fatal();
            let restart = outcome.restart_requested();
            (outcome.into_result(), timeout, fatal, restart)
        }
        Err(error) => (
            Err(error),
            shutdown::ShutdownTimeouts::defaults().runtime_shutdown_timeout(),
            false,
            false,
        ),
    };
    let fatal_message = fatal.then(|| {
        result.as_ref().err().map_or_else(
            || "unknown shutdown failure".to_owned(),
            |error| format!("{error:#}"),
        )
    });

    runtime.shutdown_timeout(timeout);
    if let Some(message) = fatal_message {
        eprintln!("any2api terminated after incomplete shutdown: {message}");
        std::process::exit(1);
    }
    if restart && result.is_ok() {
        drop(instance_lock);
        return restart_process(&executable_path, &arguments);
    }
    result
}

#[cfg(unix)]
fn restart_process(executable_path: &Path, arguments: &[OsString]) -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;

    let error = Command::new(executable_path).args(arguments).exec();
    Err(error).with_context(|| {
        format!(
            "failed to restart updated executable {}",
            executable_path.display()
        )
    })
}

#[cfg(not(unix))]
fn restart_process(_executable_path: &Path, _arguments: &[OsString]) -> anyhow::Result<()> {
    anyhow::bail!("in-place restart is unavailable on this platform")
}
