use std::{
    io::{self, Write},
    sync::{Arc, RwLock},
};

use any2api_domain::FileLogLevel;
use anyhow::Context;
use tracing_appender::non_blocking::NonBlocking;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

use super::file::FileLevelFilter;

#[derive(Clone, Default)]
struct DeferredFileWriter {
    writer: Arc<RwLock<Option<NonBlocking>>>,
}

pub(crate) struct BootstrapTracing {
    writer: DeferredFileWriter,
    level_filter: FileLevelFilter,
}

pub(super) struct FileLayerAttachment {
    writer: DeferredFileWriter,
    level_filter: FileLevelFilter,
}

impl BootstrapTracing {
    pub(crate) fn install() -> anyhow::Result<Self> {
        let (bootstrap, file) = Self::file_layer();
        let console_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let console = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(console_filter);
        tracing_subscriber::registry()
            .with(file)
            .with(console)
            .try_init()
            .context("failed to install tracing subscriber")?;
        Ok(bootstrap)
    }

    pub(super) fn attach(
        self,
        writer: NonBlocking,
        level: FileLogLevel,
    ) -> (FileLayerAttachment, FileLevelFilter) {
        self.writer.replace(Some(writer));
        self.level_filter.set(level);
        let attachment = FileLayerAttachment {
            writer: self.writer,
            level_filter: self.level_filter.clone(),
        };
        (attachment, self.level_filter)
    }

    fn file_layer() -> (Self, impl Layer<tracing_subscriber::Registry>) {
        let writer = DeferredFileWriter::default();
        let level_filter = FileLevelFilter::disabled();
        let file = tracing_subscriber::fmt::layer()
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_ansi(false)
            .with_writer(writer.clone())
            .with_filter(level_filter.clone());
        (
            Self {
                writer,
                level_filter,
            },
            file,
        )
    }

    #[cfg(test)]
    pub(super) fn for_test() -> (Self, impl Layer<tracing_subscriber::Registry>) {
        Self::file_layer()
    }
}

impl Drop for FileLayerAttachment {
    fn drop(&mut self) {
        self.level_filter.disable();
        self.writer.replace(None);
    }
}

impl DeferredFileWriter {
    fn replace(&self, writer: Option<NonBlocking>) {
        *self.writer.write().expect("deferred file writer") = writer;
    }
}

impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for DeferredFileWriter {
    type Writer = OptionalFileWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        OptionalFileWriter(self.writer.read().expect("deferred file writer").clone())
    }
}

struct OptionalFileWriter(Option<NonBlocking>);

impl Write for OptionalFileWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .as_mut()
            .map_or(Ok(bytes.len()), |writer| writer.write(bytes))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.as_mut().map_or(Ok(()), Write::flush)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use any2api_domain::FileLogLevel;
    use tempfile::tempdir;
    use tracing_appender::non_blocking::NonBlockingBuilder;
    use tracing_subscriber::layer::SubscriberExt;

    use super::BootstrapTracing;

    #[test]
    fn file_writer_can_be_attached_and_removed_without_reinstalling_subscriber() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("events.jsonl");
        let output = fs::File::create(&path).expect("output file");
        let (writer, guard) = NonBlockingBuilder::default().finish(output);
        let (bootstrap, file) = BootstrapTracing::for_test();
        let subscriber = tracing_subscriber::registry().with(file);
        let (attachment, _filter) = bootstrap.attach(writer, FileLogLevel::Info);

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(marker = "attached", "file event");
            drop(attachment);
            tracing::info!(marker = "detached", "file event");
        });
        drop(guard);

        let output = fs::read_to_string(path).expect("file output");
        assert!(output.contains("attached"), "{output}");
        assert!(!output.contains("detached"), "{output}");
    }
}
