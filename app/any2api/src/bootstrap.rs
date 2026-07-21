use std::sync::Arc;

use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AdminAuthService, AdminNetworkPolicy, AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use anyhow::Context;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::{
    admin_auth_adapter::SqliteAdminCredentialStore, build_public_request_components_with_telemetry,
    settings::AppSettings, shutdown,
};

pub async fn run() -> anyhow::Result<()> {
    initialize_tracing();
    let settings = AppSettings::from_env()?;
    let storage = Arc::new(
        SqliteStore::connect_with_master_key(&settings.database_path, &settings.master_key_path)
            .await
            .context("failed to initialize sqlite storage")?,
    );
    let configuration = storage
        .load_configuration()
        .await
        .context("failed to load configuration")?;
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
    ));
    let admin_auth = Arc::new(
        AdminAuthService::load(Arc::new(SqliteAdminCredentialStore::new(Arc::clone(
            &storage,
        ))))
        .await
        .context("failed to load administrator authentication")?,
    );
    if let Some(password) = settings.admin_password.as_ref() {
        let initialized = admin_auth
            .initialize_if_missing(password.expose_secret().to_owned())
            .await
            .context("failed to initialize administrator password")?;
        if initialized {
            tracing::info!("administrator password initialized from environment");
        }
    }
    if let Some(setup_token) = admin_auth.setup_token().await {
        eprintln!(
            "any2api administrator setup token: {setup_token}\n\
             enter this one-time token in the local web UI"
        );
    }
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = Arc::new(
        ConfigPublisher::new(storage, Arc::clone(&snapshots), Arc::clone(&runtime))
            .with_telemetry(Arc::clone(&telemetry)),
    );
    let request_components = build_public_request_components_with_telemetry(Arc::clone(&telemetry))
        .context("failed to initialize public request adapters")?;
    let public_requests = request_components.service();
    let proxy_tests = request_components.proxy_test_service();
    let app = build_router(
        AppState::new(snapshots, runtime, publisher, public_requests)
            .with_proxy_tests(proxy_tests)
            .with_request_telemetry(Arc::clone(&telemetry))
            .with_admin_auth(
                admin_auth,
                AdminNetworkPolicy::new(settings.trusted_proxy_cidrs.clone()),
            ),
        settings.web_root,
    );
    let listener = TcpListener::bind(settings.bind)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind))?;

    tracing::info!(address = %settings.bind, "any2api is listening");
    let result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown::signal())
    .await
    .context("http server failed");
    telemetry.shutdown(std::time::Duration::from_secs(5)).await;
    result
}

fn initialize_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
