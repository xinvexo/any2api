mod level_filter;
mod policy;
mod writer;

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use any2api_domain::{ConfigRevision, LoggingSettings};
use any2api_runtime::api::LoggingSettingsReconciler;
use anyhow::Context;
use level_filter::FileLevelFilter;
use policy::{FileLogPolicy, update_policy};
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, prelude::*, util::SubscriberInitExt};
use writer::RotatingFileWriter;

const FILE_LOG_QUEUE_CAPACITY: usize = 4_096;

pub(crate) struct FileLogging {
    policy: Arc<RwLock<FileLogPolicy>>,
    level_filter: FileLevelFilter,
    _guard: WorkerGuard,
}

impl FileLogging {
    pub(crate) fn initialize(
        directory: PathBuf,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> anyhow::Result<Arc<Self>> {
        let policy = Arc::new(RwLock::new(FileLogPolicy::from_settings(
            revision, settings,
        )));
        let writer = RotatingFileWriter::new(directory, Arc::clone(&policy))
            .context("failed to initialize local file logging")?;
        let (non_blocking, guard) = NonBlockingBuilder::default()
            .buffered_lines_limit(FILE_LOG_QUEUE_CAPACITY)
            .lossy(true)
            .thread_name("any2api-file-logging")
            .finish(writer);
        let level_filter = FileLevelFilter::new(settings.file_level());

        let console_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let console = tracing_subscriber::fmt::layer().with_filter(console_filter);
        let file = tracing_subscriber::fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_ansi(false)
            .with_writer(non_blocking)
            .with_filter(level_filter.clone());
        tracing_subscriber::registry()
            .with(console)
            .with(file)
            .try_init()
            .context("failed to install tracing subscriber")?;

        Ok(Arc::new(Self {
            policy,
            level_filter,
            _guard: guard,
        }))
    }
}

impl LoggingSettingsReconciler for FileLogging {
    fn reconcile(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        let next = FileLogPolicy::from_settings(revision, settings);
        if update_policy(&self.policy, next) {
            self.level_filter.set(settings.file_level());
        }
    }
}

#[cfg(test)]
mod tests;
