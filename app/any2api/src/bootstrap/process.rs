use std::{ffi::OsString, path::Path};

#[cfg(unix)]
use std::process::Command;

use any2api_updater::api::{
    APPLICATION_VERSION, RestartKind, StartupUpdateRecovery, recover_pending_update,
};
use anyhow::Context;

use super::{
    allocator_runtime, application,
    environment::StartupSettings,
    instance_lock::{self, InstanceLock},
};
use crate::{logging::BootstrapTracing, shutdown};

const UPDATE_RESTART_ENV: &str = "ANY2API_INTERNAL_UPDATE_RESTART";

pub fn run() -> anyhow::Result<()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    match command_line(&arguments) {
        Ok(CommandLine::Version) => {
            println!("any2api {APPLICATION_VERSION}");
            return Ok(());
        }
        Ok(CommandLine::Serve) => {}
        Err(error) => return recover_early_update_failure(error, &arguments),
    }
    let bootstrap_tracing = match BootstrapTracing::install() {
        Ok(tracing) => tracing,
        Err(error) => return recover_early_update_failure(error, &arguments),
    };

    let executable_path = match executable_path() {
        Ok(path) => path,
        Err(error) => return report_and_recover_early(error, &arguments),
    };
    let settings = match StartupSettings::from_env() {
        Ok(settings) => settings,
        Err(error) => return report_and_recover_early(error, &arguments),
    };
    let data_directory = match settings
        .database_path
        .parent()
        .context("database path must have a data directory")
    {
        Ok(directory) => directory,
        Err(error) => return report_and_recover_early(error, &arguments),
    };
    let instance_lock = match InstanceLock::acquire(data_directory) {
        Ok(lock) => lock,
        Err(error) if update_restart_requested() && !instance_lock::is_contention(&error) => {
            report_startup_failure("instance_lock", &error);
            return recover_early_update_failure(error, &arguments);
        }
        Err(error) => {
            report_startup_failure("instance_lock", &error);
            return Err(error);
        }
    };
    let mut recovery = discover_recovery(&executable_path);
    if let Some(pending) = recovery.take_if(|pending| !pending.target_matches(APPLICATION_VERSION))
    {
        let error = anyhow::anyhow!("pending update version does not match this binary");
        report_startup_failure("update_recovery", &error);
        return rollback_and_restart(pending, &executable_path, &arguments, instance_lock, error);
    }

    let runtime = match build_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            report_startup_failure("tokio_runtime", &error);
            return recover_locked_startup_failure(
                error,
                recovery,
                &executable_path,
                &arguments,
                instance_lock,
            );
        }
    };
    let started = runtime.block_on(application::run(
        settings,
        executable_path.clone(),
        recovery.as_mut(),
        bootstrap_tracing,
    ));
    let outcome = match started {
        Ok(outcome) => outcome,
        Err(error) => {
            report_startup_failure("application", &error);
            runtime.shutdown_timeout(
                shutdown::ShutdownTimeouts::defaults().runtime_shutdown_timeout(),
            );
            return recover_locked_startup_failure(
                error,
                recovery,
                &executable_path,
                &arguments,
                instance_lock,
            );
        }
    };
    let timeout = outcome.runtime_shutdown_timeout();
    let fatal = outcome.is_fatal();
    let restart = outcome.restart_kind();
    let result = outcome.into_result();
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
    if let (Some(kind), Ok(())) = (restart, &result) {
        drop(instance_lock);
        return restart_process(&executable_path, &arguments, kind);
    }
    result
}

fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    allocator_runtime::install(&mut builder);
    if let Some(value) = std::env::var("ANY2API_WORKER_THREADS")
        .ok()
        .filter(|value| !value.is_empty())
    {
        let workers = value
            .parse::<std::num::NonZeroUsize>()
            .context("ANY2API_WORKER_THREADS must be a positive integer")?;
        builder.worker_threads(workers.get());
    }
    builder
        .build()
        .context("failed to initialize Tokio runtime")
}

fn report_and_recover_early(error: anyhow::Error, arguments: &[OsString]) -> anyhow::Result<()> {
    report_startup_failure("bootstrap", &error);
    recover_early_update_failure(error, arguments)
}

fn report_startup_failure(phase: &'static str, error: &anyhow::Error) {
    tracing::error!(
        phase,
        error = %format_args!("{error:#}"),
        "any2api startup failed"
    );
}

enum CommandLine {
    Serve,
    Version,
}

fn command_line(arguments: &[OsString]) -> anyhow::Result<CommandLine> {
    match arguments {
        [] => Ok(CommandLine::Serve),
        [argument] if argument == "--version" => Ok(CommandLine::Version),
        _ => anyhow::bail!("usage: any2api [--version]"),
    }
}

fn executable_path() -> anyhow::Result<std::path::PathBuf> {
    match std::env::current_exe() {
        Ok(path) => Ok(path),
        Err(error) if update_restart_requested() => Err(error)
            .context("updated any2api could not resolve its executable path for startup recovery"),
        Err(error) => {
            eprintln!(
                "any2api could not resolve its executable path; self-update is disabled: {error}"
            );
            Ok(std::path::PathBuf::new())
        }
    }
}

fn discover_recovery(executable_path: &Path) -> Option<StartupUpdateRecovery> {
    if executable_path.as_os_str().is_empty() {
        return None;
    }
    match StartupUpdateRecovery::discover(executable_path, APPLICATION_VERSION) {
        Ok(recovery) => recovery,
        Err(error) => {
            eprintln!(
                "any2api ignored invalid pending update state; automatic recovery is unavailable: \
                 {error}"
            );
            None
        }
    }
}

fn recover_early_update_failure(
    startup_error: anyhow::Error,
    arguments: &[OsString],
) -> anyhow::Result<()> {
    if !update_restart_requested() {
        return Err(startup_error);
    }
    let executable_path = std::env::current_exe()
        .context("failed to resolve updated executable while recovering startup")?;
    match recover_pending_update(&executable_path, APPLICATION_VERSION) {
        Ok(true) => {
            eprintln!(
                "updated any2api failed before startup ownership was established: \
                 {startup_error:#}; restored previous executable"
            );
            exec_process(&executable_path, arguments, false)
        }
        Ok(false) => Err(startup_error),
        Err(recovery_error) => Err(startup_error.context(format!(
            "automatic binary rollback also failed: {recovery_error}"
        ))),
    }
}

fn recover_locked_startup_failure(
    startup_error: anyhow::Error,
    recovery: Option<StartupUpdateRecovery>,
    executable_path: &Path,
    arguments: &[OsString],
    instance_lock: InstanceLock,
) -> anyhow::Result<()> {
    let Some(recovery) = recovery.filter(|pending| !pending.is_confirmed()) else {
        return Err(startup_error);
    };
    rollback_and_restart(
        recovery,
        executable_path,
        arguments,
        instance_lock,
        startup_error,
    )
}

fn rollback_and_restart(
    recovery: StartupUpdateRecovery,
    executable_path: &Path,
    arguments: &[OsString],
    instance_lock: InstanceLock,
    startup_error: anyhow::Error,
) -> anyhow::Result<()> {
    recovery.rollback().with_context(|| {
        format!("updated any2api failed to start ({startup_error:#}); binary rollback failed")
    })?;
    eprintln!("updated any2api failed to start: {startup_error:#}; restored previous executable");
    drop(instance_lock);
    exec_process(executable_path, arguments, false)
}

fn update_restart_requested() -> bool {
    std::env::var_os(UPDATE_RESTART_ENV).is_some_and(|value| value == "1")
}

fn restart_process(
    executable_path: &Path,
    arguments: &[OsString],
    kind: RestartKind,
) -> anyhow::Result<()> {
    match kind {
        RestartKind::Manual => exec_process(
            executable_path,
            arguments,
            restart_uses_update_recovery(kind),
        ),
        RestartKind::Update => restart_updated_process(executable_path, arguments, kind),
    }
}

const fn restart_uses_update_recovery(kind: RestartKind) -> bool {
    matches!(kind, RestartKind::Update)
}

#[cfg(unix)]
fn restart_updated_process(
    executable_path: &Path,
    arguments: &[OsString],
    kind: RestartKind,
) -> anyhow::Result<()> {
    let error = exec_error(
        executable_path,
        arguments,
        restart_uses_update_recovery(kind),
    );
    let restart_error = anyhow::Error::new(error).context(format!(
        "failed to restart updated executable {}",
        executable_path.display()
    ));
    match recover_pending_update(executable_path, APPLICATION_VERSION) {
        Ok(true) => {
            eprintln!("{restart_error:#}; restored previous executable");
            exec_process(executable_path, arguments, false)
        }
        Ok(false) => Err(restart_error),
        Err(recovery_error) => Err(restart_error.context(format!(
            "automatic binary rollback also failed: {recovery_error}"
        ))),
    }
}

#[cfg(not(unix))]
fn restart_updated_process(
    _executable_path: &Path,
    _arguments: &[OsString],
    _kind: RestartKind,
) -> anyhow::Result<()> {
    anyhow::bail!("in-place restart is unavailable on this platform")
}

#[cfg(unix)]
fn exec_process(
    executable_path: &Path,
    arguments: &[OsString],
    update_restart: bool,
) -> anyhow::Result<()> {
    let error = exec_error(executable_path, arguments, update_restart);
    Err(error).with_context(|| format!("failed to execute {}", executable_path.display()))
}

#[cfg(unix)]
fn exec_error(
    executable_path: &Path,
    arguments: &[OsString],
    update_restart: bool,
) -> std::io::Error {
    use std::os::unix::process::CommandExt;

    let mut command = Command::new(executable_path);
    command.args(arguments);
    if update_restart {
        command.env(UPDATE_RESTART_ENV, "1");
    } else {
        command.env_remove(UPDATE_RESTART_ENV);
    }
    command.exec()
}

#[cfg(not(unix))]
fn exec_process(
    _executable_path: &Path,
    _arguments: &[OsString],
    _update_restart: bool,
) -> anyhow::Result<()> {
    anyhow::bail!("in-place process execution is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use any2api_updater::api::RestartKind;

    use super::{CommandLine, command_line, restart_uses_update_recovery};

    #[test]
    fn command_line_accepts_only_service_or_exact_version_mode() {
        assert!(matches!(command_line(&[]).unwrap(), CommandLine::Serve));
        assert!(matches!(
            command_line(&["--version".into()]).unwrap(),
            CommandLine::Version
        ));
        assert!(command_line(&["--help".into()]).is_err());
        assert!(command_line(&["--version".into(), "extra".into()]).is_err());
    }

    #[test]
    fn only_update_restart_uses_update_recovery_environment() {
        assert!(!restart_uses_update_recovery(RestartKind::Manual));
        assert!(restart_uses_update_recovery(RestartKind::Update));
    }
}
