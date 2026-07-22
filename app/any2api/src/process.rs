use anyhow::Context;

use crate::{bootstrap, instance_lock::InstanceLock, settings::AppSettings, shutdown};

pub fn run() -> anyhow::Result<()> {
    let settings = AppSettings::from_env()?;
    let data_directory = settings
        .database_path
        .parent()
        .context("database path must have a data directory")?;
    let _instance_lock = InstanceLock::acquire(data_directory)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to initialize Tokio runtime")?;

    let started = runtime.block_on(bootstrap::run(settings));
    let (result, timeout, fatal) = match started {
        Ok(outcome) => {
            let timeout = outcome.runtime_shutdown_timeout();
            let fatal = outcome.is_fatal();
            (outcome.into_result(), timeout, fatal)
        }
        Err(error) => (
            Err(error),
            shutdown::ShutdownTimeouts::defaults().runtime_shutdown_timeout(),
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
    result
}
