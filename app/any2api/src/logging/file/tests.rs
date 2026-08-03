use std::{fs, io::Write, sync::RwLock};

use any2api_domain::{ConfigRevision, FileLogLevel};
use tempfile::tempdir;
use tracing::Level;
use tracing_appender::non_blocking::NonBlockingBuilder;
use tracing_subscriber::{layer::SubscriberExt, prelude::*};

use super::{
    FileLevelFilter, FileLogging, FileLoggingControl,
    policy::FileLogPolicy,
    writer::{RotatingFileWriter, managed_files},
};

#[test]
fn control_clone_does_not_retain_the_worker_guard() {
    let directory = tempdir().expect("temporary directory");
    let path = directory.path().join("queued.log");
    let file = fs::File::create(&path).expect("log file");
    let (mut non_blocking, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(16)
        .lossy(true)
        .finish(file);
    let level_filter = FileLevelFilter::disabled();
    level_filter.set(FileLogLevel::Info);
    let control = FileLoggingControl {
        policy: std::sync::Arc::new(RwLock::new(FileLogPolicy {
            revision: ConfigRevision::INITIAL,
            retention_secs: 86_400,
            max_total_size: 1024 * 1024,
        })),
        level_filter,
    };
    let retained_control = control.clone();
    let logging = FileLogging {
        control,
        _attachment: None,
        _guard: guard,
    };
    writeln!(non_blocking, "queued before shutdown").expect("queue log line");

    FileLogging::finish(logging);

    assert_eq!(
        fs::read_to_string(path).expect("flushed log"),
        "queued before shutdown\n"
    );
    drop(retained_control);
}

#[test]
fn level_filter_updates_immediately() {
    let filter = FileLevelFilter::disabled();
    assert!(!filter.enabled_level(&Level::ERROR));
    filter.set(FileLogLevel::Info);
    assert!(filter.enabled_level(&Level::ERROR));
    assert!(filter.enabled_level(&Level::INFO));
    assert!(!filter.enabled_level(&Level::DEBUG));

    filter.set(FileLogLevel::Trace);
    assert!(filter.enabled_level(&Level::DEBUG));
    assert!(filter.enabled_level(&Level::TRACE));
}

#[test]
fn tracing_events_are_written_as_json_lines() {
    let directory = tempdir().expect("temporary directory");
    let policy = std::sync::Arc::new(RwLock::new(FileLogPolicy {
        revision: ConfigRevision::INITIAL,
        retention_secs: 86_400,
        max_total_size: 1024 * 1024,
    }));
    let writer =
        RotatingFileWriter::new(directory.path().to_path_buf(), policy).expect("rotating writer");
    let (non_blocking, guard) = NonBlockingBuilder::default()
        .buffered_lines_limit(16)
        .lossy(true)
        .finish(writer);
    let level_filter = FileLevelFilter::disabled();
    level_filter.set(FileLogLevel::Info);
    let layer = tracing_subscriber::fmt::layer()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_ansi(false)
        .with_writer(non_blocking)
        .with_filter(level_filter);
    let subscriber = tracing_subscriber::registry().with(layer);

    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(answer = 42, "written");
        tracing::debug!("filtered");
    });
    drop(guard);

    let files = managed_files(directory.path(), None).expect("managed files");
    assert_eq!(files.len(), 1);
    let content = fs::read_to_string(&files[0].path).expect("log content");
    let lines = content.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let event: serde_json::Value = serde_json::from_str(lines[0]).expect("json event");
    assert_eq!(event["message"], "written");
    assert_eq!(event["answer"], 42);
}
