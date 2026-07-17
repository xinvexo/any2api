use std::sync::Arc;

use any2api_runtime::api::{ConfigPublisher, PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::{settings::AppSettings, shutdown};

pub(crate) async fn run() -> anyhow::Result<()> {
    initialize_tracing();
    let settings = AppSettings::from_env()?;
    let storage = Arc::new(
        SqliteStore::connect(&settings.database_path)
            .await
            .context("failed to initialize sqlite storage")?,
    );
    let configuration = storage
        .load_configuration()
        .await
        .context("failed to load configuration")?;
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration.revision(),
        configuration.proxies().clone(),
    )));
    let runtime = Arc::new(RuntimeRegistry::new());
    let publisher = Arc::new(ConfigPublisher::new(
        storage,
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    ));
    let app = build_router(
        AppState::new(snapshots, runtime, publisher),
        settings.web_root,
    );
    let listener = TcpListener::bind(settings.bind)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind))?;

    tracing::info!(address = %settings.bind, "any2api is listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::signal())
    .await
    .context("http server failed")
}

fn initialize_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
