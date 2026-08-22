use std::{path::PathBuf, sync::Arc, time::Duration};

use any2api_runtime::api::{
    ConfigPublisher, OAuthService, OfficialClientVersionService, PublishedSnapshot,
    RequestTelemetry, RuntimeRegistry, SnapshotStore,
};
use any2api_server::api::{AdminAuthService, AppServices, AppState, WebAssets, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_updater::api::{GitHubReleaseUpdater, StartupUpdateRecovery};
use anyhow::Context;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;

use super::{
    admin_credentials::SqliteAdminCredentialStore, environment::StartupSettings, listener,
    native_memory_reclamation,
    public_request_components::build_public_request_components_with_telemetry, web_assets,
};
use crate::{
    logging::{AppSnapshotReconciler, BootstrapTracing, FileLogging},
    self_update::{LifecycleUpdateTaskExecutor, RestartSignal},
    shutdown,
};

pub(super) async fn run(
    settings: StartupSettings,
    executable_path: PathBuf,
    startup_recovery: Option<&mut StartupUpdateRecovery>,
    bootstrap_tracing: BootstrapTracing,
) -> anyhow::Result<shutdown::ShutdownOutcome> {
    let storage = Arc::new(
        SqliteStore::connect(&settings.database_path)
            .await
            .context("failed to initialize sqlite storage")?,
    );
    let configuration = storage
        .load_configuration()
        .await
        .context("failed to load configuration")?;
    let max_connections = configuration.settings().network().max_connections();
    let file_logging = FileLogging::initialize(
        bootstrap_tracing,
        settings.log_directory.clone(),
        configuration.revision(),
        configuration.settings().logging(),
    )?;
    let runtime = Arc::new(RuntimeRegistry::new());
    let lifecycle = runtime.lifecycle();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&storage),
        configuration.revision(),
        configuration.settings().logging(),
        &lifecycle,
    ));
    let admin_auth = Arc::new(
        AdminAuthService::load(
            Arc::new(SqliteAdminCredentialStore::new(Arc::clone(&storage))),
            lifecycle.clone(),
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
    let request_components = build_public_request_components_with_telemetry(Arc::clone(&telemetry))
        .context("failed to initialize public request adapters")?;
    let configuration_capabilities = request_components.configuration_capabilities();
    let snapshots = Arc::new(SnapshotStore::new(
        PublishedSnapshot::new(
            configuration,
            runtime.as_ref(),
            request_components.provider_registry(),
        )
        .context("failed to compile the stored configuration")?,
    ));
    let official_client_versions = OfficialClientVersionService::new(
        request_components.provider_registry_handle(),
        request_components.transport_manager(),
        Arc::clone(&snapshots),
        Arc::clone(&storage),
    );
    official_client_versions
        .initialize()
        .await
        .context("failed to initialize official client versions")?;
    let snapshot_reconciler = Arc::new(AppSnapshotReconciler::new(
        Arc::clone(&telemetry),
        file_logging.control(),
    ));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            configuration_capabilities,
        )
        .context("loaded configuration is incompatible with registered providers or protocols")?
        .with_snapshot_reconciler(snapshot_reconciler),
    );
    let oauth = Arc::new(OAuthService::new(
        request_components.provider_registry_handle(),
        request_components.transport_manager(),
        Arc::clone(&publisher),
        Arc::clone(&storage),
        Arc::clone(&telemetry),
    ));
    let public_requests = request_components
        .service_with_oauth(oauth.as_ref())
        .context("failed to initialize OAuth-aware public request service")?;
    let proxy_tests = request_components.proxy_test_service();
    let provider_credential_tests = request_components.provider_credential_test_service();
    let embedded_web = settings.web_root.is_none();
    let restart = RestartSignal::new(cfg!(unix) && !executable_path.as_os_str().is_empty());
    let update_tasks = Arc::new(LifecycleUpdateTaskExecutor::new(lifecycle.clone()));
    let application_updates = Arc::new(
        GitHubReleaseUpdater::official(
            executable_path,
            embedded_web,
            Arc::new(restart.clone()),
            update_tasks,
        )
        .await
        .context("failed to initialize application updater")?,
    );
    let web_assets = settings
        .web_root
        .map(WebAssets::external)
        .unwrap_or_else(web_assets::assets);
    let app = build_router(
        AppState::production(
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            Arc::clone(&publisher),
            public_requests,
            admin_auth,
            AppServices::new(
                Arc::clone(&oauth),
                proxy_tests,
                provider_credential_tests,
                Arc::clone(&telemetry),
                application_updates,
                Arc::new(restart.clone()),
            ),
        ),
        web_assets,
    );
    let listener = TcpListener::bind(settings.bind)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind))?;
    let listener = listener::inbound_listener(listener, max_connections);

    native_memory_reclamation::start(&lifecycle);
    anyhow::ensure!(
        oauth.start_refresh_worker(&lifecycle),
        "OAuth refresh worker was already started"
    );
    anyhow::ensure!(
        oauth.start_quota_worker(&lifecycle),
        "OAuth quota activity worker was already started"
    );
    anyhow::ensure!(
        official_client_versions.start(&lifecycle),
        "official client version worker was already started"
    );
    anyhow::ensure!(
        runtime.start_affinity_sweeper(publisher),
        "affinity sweeper was already started"
    );
    let shutdown_signal = shutdown::ShutdownSignal::install()
        .context("failed to install process shutdown signal handlers")?;
    if let Some(recovery) = startup_recovery {
        recovery.confirm_startup();
    }
    tracing::info!(address = %settings.bind, "any2api is listening");
    let served = shutdown::serve(
        listener,
        app,
        lifecycle.clone(),
        snapshots.as_ref(),
        Duration::from_secs(
            snapshots
                .load()
                .settings()
                .network()
                .request_header_timeout_secs(),
        ),
        async {
            tokio::select! {
                () = shutdown_signal.wait() => {}
                () = restart.wait() => {}
            }
        },
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

    // Release service roots that retain the configuration publisher and snapshot reconciler.
    drop(request_components);
    drop(oauth);
    match &finalized {
        Ok(()) => tracing::info!("any2api shutdown complete"),
        Err(error) => {
            tracing::error!(?error, "any2api shutdown incomplete; terminating process");
        }
    }
    let outcome = shutdown::ShutdownOutcome::after_finalization(result, finalized, served.timeouts);
    FileLogging::finish(file_logging);
    Ok(outcome.with_restart_kind(restart.kind()))
}
