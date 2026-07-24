use std::{sync::Arc, time::Duration};

use any2api_domain::{ConfigRevision, SettingKey, SettingValue};
use any2api_runtime::api::{
    ConfigPublisher, ProcessLifecycle, PublishedSnapshot, RequestTelemetry, RuntimeRegistry,
    ShutdownPhase, SnapshotStore,
};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use axum::{Router, routing::get};
use tokio::{
    net::TcpListener,
    sync::{Notify, oneshot},
};

use super::{ShutdownTimeouts, finalization::finalize, server::serve_with_timeout_source};

#[tokio::test]
async fn injected_signal_stops_accepting_and_waits_for_an_active_handler() {
    let lifecycle = ProcessLifecycle::new();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let handler_started = Arc::clone(&started);
    let handler_release = Arc::clone(&release);
    let app = Router::new().route(
        "/",
        get(move || {
            let started = Arc::clone(&handler_started);
            let release = Arc::clone(&handler_release);
            async move {
                started.notify_one();
                release.notified().await;
                "done"
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let address = listener.local_addr().expect("address");
    let (signal_sender, signal_receiver) = oneshot::channel();
    let server_lifecycle = lifecycle.clone();
    let server = tokio::spawn(async move {
        serve_with_timeout_source(
            listener,
            app,
            server_lifecycle,
            || test_timeouts(1_000, 1_000),
            async move {
                signal_receiver.await.ok();
            },
        )
        .await
        .result
    });
    let client = tokio::spawn(async move {
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("client connect");
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await
            .expect("request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.expect("response");
        response
    });

    started.notified().await;
    signal_sender.send(()).expect("send signal");
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!server.is_finished());
    release.notify_waiters();

    server.await.expect("server task").expect("server result");
    let response = client.await.expect("client task");
    assert!(String::from_utf8_lossy(&response).contains("200 OK"));
}

#[tokio::test]
async fn finalization_forces_a_stalled_tracked_task() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let storage = SqliteStore::connect(&directory.path().join("shutdown.sqlite3"))
        .await
        .expect("storage");
    let lifecycle = ProcessLifecycle::new();
    let _task = lifecycle.spawn_critical(std::future::pending::<()>());
    let telemetry = RequestTelemetry::disabled();
    let timeouts = short_timeouts();

    finalize(&lifecycle, &telemetry, &storage, timeouts)
        .await
        .expect("shutdown finalization");

    assert_eq!(lifecycle.phase(), ShutdownPhase::Forced);
    assert_eq!(lifecycle.background_task_count(), 0);
}

#[tokio::test]
async fn signal_captures_the_latest_published_shutdown_settings() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("settings.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let components = crate::build_public_request_components().expect("public request components");
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
        components.provider_registry(),
    )));
    let publisher = ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
        components.configuration_capabilities(),
    )
    .expect("configuration publisher");
    publisher
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::ShutdownRequestGracePeriod,
            SettingValue::DurationSecs(1),
        )
        .await
        .expect("publish shutdown setting");

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("listener");
    let (signal_sender, signal_receiver) = oneshot::channel();
    let outcome = super::server::serve(
        listener,
        Router::new(),
        runtime.lifecycle(),
        snapshots.as_ref(),
        async move {
            signal_receiver.await.ok();
        },
    );
    signal_sender.send(()).expect("send signal");
    let outcome = outcome.await;

    outcome.result.expect("server result");
    assert_eq!(outcome.timeouts.request_grace, Duration::from_secs(1));
    storage.close().await;
}

#[tokio::test]
async fn finalization_reports_a_blocking_task_that_misses_the_deadline() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let storage = SqliteStore::connect(&directory.path().join("blocked.sqlite3"))
        .await
        .expect("storage");
    let lifecycle = ProcessLifecycle::new();
    let (started_sender, started_receiver) = std::sync::mpsc::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();
    let task = lifecycle.spawn_blocking(move || {
        started_sender.send(()).expect("started");
        release_receiver.recv().expect("release");
    });
    started_receiver.recv().expect("blocking task started");

    let error = finalize(
        &lifecycle,
        &RequestTelemetry::disabled(),
        &storage,
        short_timeouts(),
    )
    .await
    .expect_err("stalled blocking task must fail finalization");
    assert!(error.to_string().contains("background tasks did not stop"));
    assert_eq!(lifecycle.phase(), ShutdownPhase::Forced);

    release_sender.send(()).expect("release blocking task");
    task.await.expect("blocking task");
    lifecycle.wait_for_background_tasks().await;
    storage.close().await;
}

fn test_timeouts(request_grace_secs: u64, finalize_secs: u64) -> ShutdownTimeouts {
    ShutdownTimeouts {
        request_grace: Duration::from_secs(request_grace_secs),
        finalize: Duration::from_secs(finalize_secs),
    }
}

fn short_timeouts() -> ShutdownTimeouts {
    test_timeouts(10, 10)
}
