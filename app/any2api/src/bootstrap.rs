use std::sync::Arc;

use any2api_runtime::api::{
    ConfigPublisher, PublishedSnapshot, RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AdminAuthService, AdminNetworkPolicy, AppState, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use anyhow::Context;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;

use crate::{
    admin_auth_adapter::SqliteAdminCredentialStore, build_public_request_components_with_telemetry,
    file_logging::FileLogging, logging_reconciler::AppLoggingReconciler, settings::AppSettings,
    shutdown,
};

pub(crate) async fn run(settings: AppSettings) -> anyhow::Result<shutdown::ShutdownOutcome> {
    let storage = Arc::new(
        SqliteStore::connect_with_master_key(&settings.database_path, &settings.master_key_path)
            .await
            .context("failed to initialize sqlite storage")?,
    );
    let configuration = storage
        .load_configuration()
        .await
        .context("failed to load configuration")?;
    let file_logging = FileLogging::initialize(
        settings.log_directory.clone(),
        configuration.revision(),
        configuration.settings().logging(),
    )?;
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &runtime.lifecycle(),
    ));
    let admin_auth = Arc::new(
        AdminAuthService::load(
            Arc::new(SqliteAdminCredentialStore::new(Arc::clone(&storage))),
            runtime.lifecycle(),
        )
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
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let logging_reconciler = Arc::new(AppLoggingReconciler::new(
        Arc::clone(&telemetry),
        Arc::clone(&file_logging),
    ));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
        )
        .with_logging_reconciler(logging_reconciler),
    );
    let request_components = build_public_request_components_with_telemetry(Arc::clone(&telemetry))
        .context("failed to initialize public request adapters")?;
    let public_requests = request_components.service();
    let proxy_tests = request_components.proxy_test_service();
    let app = build_router(
        AppState::new(
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            publisher,
            public_requests,
        )
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
    let lifecycle = runtime.lifecycle();
    let served = shutdown::serve(
        listener,
        app,
        lifecycle.clone(),
        snapshots.as_ref(),
        shutdown::signal(),
    )
    .await;
    let result = served.result.context("http server failed");
    let finalized = shutdown::finalize(
        &lifecycle,
        telemetry.as_ref(),
        storage.as_ref(),
        served.timeouts,
    )
    .await
    .context("shutdown finalization failed");

    let finalized = finalized.and_then(|()| FileLogging::finish(file_logging));
    let outcome = match finalized {
        Ok(()) => shutdown::ShutdownOutcome::complete(result, served.timeouts),
        Err(error) => {
            tracing::error!(?error, "any2api shutdown incomplete; terminating process");
            shutdown::ShutdownOutcome::fatal(error, served.timeouts)
        }
    };
    Ok(outcome)
}
