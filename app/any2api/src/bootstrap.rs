use std::sync::Arc;

use any2api_runtime::api::{PublishedSnapshot, RuntimeRegistry, SnapshotStore};
use any2api_server::api::{AppState, build_router};
use any2api_storage::api::SqliteStore;
use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::{settings::AppSettings, shutdown};

pub(crate) async fn run() -> anyhow::Result<()> {
    initialize_tracing();
    let settings = AppSettings::from_env()?;
    let storage = SqliteStore::connect(&settings.database_path)
        .await
        .context("failed to initialize sqlite storage")?;
    let revision = storage
        .load_config_revision()
        .await
        .context("failed to load configuration revision")?;
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(revision)));
    let runtime = Arc::new(RuntimeRegistry::new());
    let app = build_router(AppState::new(snapshots, runtime), settings.web_root);
    let listener = TcpListener::bind(settings.bind)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind))?;

    tracing::info!(address = %settings.bind, "any2api is listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown::signal())
        .await
        .context("http server failed")
}

fn initialize_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
