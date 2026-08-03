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
pub(in crate::logging) use level_filter::FileLevelFilter;
use policy::{FileLogPolicy, update_policy};
use tracing_appender::non_blocking::{NonBlockingBuilder, WorkerGuard};
use writer::RotatingFileWriter;

use super::subscriber::{BootstrapTracing, FileLayerAttachment};

const FILE_LOG_QUEUE_CAPACITY: usize = 4_096;

pub(crate) struct FileLogging {
    control: FileLoggingControl,
    // Rust drops fields in declaration order: detach before the worker guard flushes.
    _attachment: Option<FileLayerAttachment>,
    _guard: WorkerGuard,
}

#[derive(Clone)]
pub(crate) struct FileLoggingControl {
    policy: Arc<RwLock<FileLogPolicy>>,
    level_filter: FileLevelFilter,
}

impl FileLogging {
    pub(crate) fn initialize(
        bootstrap: BootstrapTracing,
        directory: PathBuf,
        revision: ConfigRevision,
        settings: &LoggingSettings,
    ) -> anyhow::Result<Self> {
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
        let (attachment, level_filter) = bootstrap.attach(non_blocking, settings.file_level());

        Ok(Self {
            control: FileLoggingControl {
                policy,
                level_filter,
            },
            _attachment: Some(attachment),
            _guard: guard,
        })
    }

    pub(crate) fn control(&self) -> FileLoggingControl {
        self.control.clone()
    }

    pub(crate) fn finish(self) {
        drop(self);
    }
}

impl LoggingSettingsReconciler for FileLoggingControl {
    fn reconcile(&self, revision: ConfigRevision, settings: &LoggingSettings) {
        let next = FileLogPolicy::from_settings(revision, settings);
        if update_policy(&self.policy, next) {
            self.level_filter.set(settings.file_level());
        }
    }
}

#[cfg(test)]
mod tests;
