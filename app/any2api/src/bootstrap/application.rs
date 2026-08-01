use std::{path::PathBuf, sync::Arc};

use any2api_runtime::api::{
    ConfigPublisher, OAuthService, PublishedSnapshot, RequestTelemetry, RuntimeRegistry,
    SnapshotStore,
};
use any2api_server::api::{AdminAuthService, AppState, WebAssets, build_router};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_updater::api::GitHubReleaseUpdater;
use anyhow::Context;
use secrecy::ExposeSecret;
use tokio::net::TcpListener;

use super::{
    admin_credentials::SqliteAdminCredentialStore, environment::StartupSettings,
    public_request_components::build_public_request_components_with_telemetry, web_assets,
};
use crate::{
    logging::{AppLoggingReconciler, FileLogging},
    self_update::{LifecycleUpdateTaskExecutor, RestartSignal},
    shutdown,
};

pub(super) async fn run(
    settings: StartupSettings,
    executable_path: PathBuf,
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
    let file_logging = FileLogging::initialize(
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
    let logging_reconciler = Arc::new(AppLoggingReconciler::new(
        Arc::clone(&telemetry),
        Arc::clone(&file_logging),
    ));
    let publisher = Arc::new(
        ConfigPublisher::new(
            Arc::clone(&storage),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            configuration_capabilities,
        )
        .context("loaded configuration is incompatible with registered providers or protocols")?
        .with_logging_reconciler(logging_reconciler),
    );
    let public_requests = request_components.service();
    let oauth = Arc::new(OAuthService::new(
        request_components.provider_registry_handle(),
        request_components.transport_manager(),
        Arc::clone(&publisher),
    ));
    anyhow::ensure!(
        public_requests.install_oauth_refresh(oauth.as_ref()),
        "OAuth request refresh was already installed"
    );
    let proxy_tests = request_components.proxy_test_service();
    let provider_credential_tests = request_components.provider_credential_test_service();
    let embedded_web = settings.web_root.is_none();
    let restart = RestartSignal::new();
    let update_tasks = Arc::new(LifecycleUpdateTaskExecutor::new(lifecycle.clone()));
    let application_updates = Arc::new(
        GitHubReleaseUpdater::official(
            executable_path,
            embedded_web,
            Arc::new(restart.clone()),
            update_tasks,
        )
        .context("failed to initialize application updater")?,
    );
    let web_assets = settings
        .web_root
        .map(WebAssets::external)
        .unwrap_or_else(web_assets::assets);
    let app = build_router(
        AppState::new(
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            Arc::clone(&publisher),
            public_requests,
        )
        .with_oauth(Arc::clone(&oauth))
        .with_proxy_tests(proxy_tests)
        .with_provider_credential_tests(provider_credential_tests)
        .with_request_telemetry(Arc::clone(&telemetry))
        .with_application_updates(application_updates)
        .with_admin_auth(admin_auth),
        web_assets,
    );
    let listener = TcpListener::bind(settings.bind)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind))?;

    anyhow::ensure!(
        oauth.start_refresh_worker(&lifecycle),
        "OAuth refresh worker was already started"
    );
    anyhow::ensure!(
        runtime.start_affinity_sweeper(publisher),
        "affinity sweeper was already started"
    );
    tracing::info!(address = %settings.bind, "any2api is listening");
    let served = shutdown::serve(
        listener,
        app,
        lifecycle.clone(),
        snapshots.as_ref(),
        async {
            tokio::select! {
                () = shutdown::signal() => {}
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

    // Release service roots that retain the configuration publisher and logging reconciler.
    drop(request_components);
    drop(oauth);
    let finalized = finalized.and_then(|()| FileLogging::finish(file_logging));
    let outcome = match finalized {
        Ok(()) => shutdown::ShutdownOutcome::complete(result, served.timeouts),
        Err(error) => {
            tracing::error!(?error, "any2api shutdown incomplete; terminating process");
            shutdown::ShutdownOutcome::fatal(error, served.timeouts)
        }
    };
    Ok(outcome.with_restart_requested(restart.requested()))
}
