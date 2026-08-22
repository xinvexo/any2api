use std::sync::Arc;

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, OAuthAccountDraft, OAuthAccountId,
    OAuthProxySelection, ProtocolDialect, ProviderCredentialDraft, ProviderEndpointDraft,
    ProviderEndpointId, ProviderKind, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfileId,
    SettingKey, SettingValue,
};
use any2api_storage::api::{
    ConfigurationRepository, OAuthAccountDocument, OAuthAccountRefresh, SqliteStore,
};
use tempfile::{TempDir, tempdir};

use crate::{
    configuration::{
        ConfigPublishError, ConfigPublisher, PublishedSnapshot, PublishedSnapshotReconciler,
        SnapshotStore,
    },
    credential::ProviderApiKeySecret,
    registry::RuntimeRegistry,
    request_telemetry::RequestTelemetry,
};

mod atomicity;
mod commit_ack;
mod gateway_usage;
mod oauth_login;
mod ordering;
mod rate_card;

#[tokio::test]
async fn settings_publish_updates_request_telemetry_policy() {
    let context = TestContext::new().await;
    let initial = context.snapshots.load();
    let telemetry = Arc::new(RequestTelemetry::start(
        Arc::clone(&context.repository),
        initial.revision(),
        initial.settings().logging(),
        &context.runtime.lifecycle(),
    ));
    let publisher = ConfigPublisher::new(
        Arc::clone(&context.repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher")
    .with_snapshot_reconciler(Arc::clone(&telemetry) as Arc<dyn PublishedSnapshotReconciler>);

    let published = publisher
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::LogsRequestMaxRows,
            SettingValue::Integer(42),
        )
        .await
        .expect("publish request log limit");
    let published = publisher
        .set_setting_override(
            published.revision(),
            SettingKey::LogsHttpAccessMaxRows,
            SettingValue::Integer(7),
        )
        .await
        .expect("publish HTTP access log row limit");
    let published = publisher
        .set_setting_override(
            published.revision(),
            SettingKey::LogsTelemetryQueueMaxBytes,
            SettingValue::Integer(8 * 1024 * 1024),
        )
        .await
        .expect("publish telemetry owned byte limit");
    let policy = telemetry.current_policy();

    assert_eq!(policy.revision, published.revision());
    assert_eq!(policy.request_max_rows, 42);
    assert_eq!(policy.http_access_max_rows, 7);
    assert_eq!(policy.queue_max_bytes, 8 * 1024 * 1024);
    telemetry.shutdown(std::time::Duration::from_secs(1)).await;
}

#[tokio::test]
async fn provider_endpoint_publish_switches_the_complete_snapshot() {
    let context = TestContext::new().await;
    let id = ProviderEndpointId::new();
    let published = context
        .publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            id,
            ProviderEndpointDraft::new(
                "Codex Primary",
                ProviderKind::Codex,
                "https://api.example.com/v1/",
                ProtocolDialect::OpenAiResponses,
                None,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("publish endpoint");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), stored.revision());
    assert!(published.provider_endpoints().get(id).is_some());
    assert!(published.proxies().profiles().len() == stored.proxies().profiles().len());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn provider_credential_publish_switches_the_complete_snapshot() {
    let context = TestContext::new().await;
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = context
        .publisher
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, codex_endpoint_draft())
        .await
        .expect("publish endpoint");

    let published = context
        .publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            ProviderApiKeySecret::new("sk-runtime-credential".to_owned()),
        )
        .await
        .expect("publish credential");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(published.revision(), stored.revision());
    assert!(
        published
            .provider_credentials()
            .get(credential_id)
            .is_some()
    );
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 2);
}

#[tokio::test]
async fn global_proxy_applies_to_oauth_but_not_direct_api_keys() {
    let context = TestContext::new().await;
    let proxy_id = ProxyProfileId::new();
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let global_oauth_account_id = OAuthAccountId::new();
    let direct_oauth_account_id = OAuthAccountId::new();
    let proxy = context
        .publisher
        .create_proxy(ConfigRevision::INITIAL, proxy_id, proxy_draft("Hong Kong"))
        .await
        .expect("publish proxy");
    let global = context
        .publisher
        .set_global_proxy(proxy.revision(), proxy_id)
        .await
        .expect("publish global proxy");
    let endpoint = context
        .publisher
        .create_provider_endpoint(global.revision(), endpoint_id, codex_endpoint_draft())
        .await
        .expect("publish endpoint");
    context
        .publisher
        .create_provider_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            credential_draft(),
            ProviderApiKeySecret::new("sk-runtime-proxy".to_owned()),
        )
        .await
        .expect("publish credential");
    context
        .publisher
        .activate_oauth_account(
            global_oauth_account_id,
            ProviderKind::Codex,
            oauth_account_draft("Global OAuth"),
            OAuthProxySelection::Global,
            None,
            None,
            Vec::new(),
            oauth_document(ProviderKind::Codex, "global-oauth-token", None),
        )
        .await
        .expect("publish global OAuth account");
    let published = context
        .publisher
        .activate_oauth_account(
            direct_oauth_account_id,
            ProviderKind::Codex,
            oauth_account_draft("Direct OAuth"),
            OAuthProxySelection::Profile(ProxyProfileId::DIRECT),
            None,
            None,
            Vec::new(),
            oauth_document(ProviderKind::Codex, "direct-oauth-token", None),
        )
        .await
        .expect("publish direct OAuth account");

    assert_eq!(
        published
            .resolved_proxy_for_credential(credential_id)
            .map(|profile| profile.id()),
        Some(ProxyProfileId::DIRECT)
    );
    assert_eq!(
        published
            .resolved_transport_proxy_for_oauth_account(global_oauth_account_id)
            .map(|proxy| proxy.profile().id()),
        Some(proxy_id)
    );
    assert_eq!(
        published
            .resolved_transport_proxy_for_oauth_account(direct_oauth_account_id)
            .map(|proxy| proxy.profile().id()),
        Some(ProxyProfileId::DIRECT)
    );
    assert_eq!(
        published
            .routing_credentials()
            .iter()
            .find(|routing| routing.id() == credential_id.into())
            .map(|routing| routing.proxy_id()),
        Some(ProxyProfileId::DIRECT)
    );
    assert_eq!(
        published
            .routing_credentials()
            .iter()
            .find(|routing| routing.id() == global_oauth_account_id.into())
            .map(|routing| routing.proxy_id()),
        Some(proxy_id)
    );
    assert_eq!(
        published
            .routing_credentials()
            .iter()
            .find(|routing| routing.id() == direct_oauth_account_id.into())
            .map(|routing| routing.proxy_id()),
        Some(ProxyProfileId::DIRECT)
    );
}

#[tokio::test]
async fn publishers_sharing_a_snapshot_store_are_serialized() {
    let context = TestContext::new().await;
    let second_publisher = ConfigPublisher::new(
        Arc::clone(&context.repository),
        Arc::clone(&context.snapshots),
        Arc::clone(&context.runtime),
        crate::test_support::configuration_capabilities(),
    )
    .expect("configuration publisher");
    let first_id = ProxyProfileId::new();
    let second_id = ProxyProfileId::new();

    let (first, second) = tokio::join!(
        context
            .publisher
            .create_proxy(ConfigRevision::INITIAL, first_id, proxy_draft("First")),
        second_publisher.create_proxy(ConfigRevision::INITIAL, second_id, proxy_draft("Second"))
    );
    let success_count = usize::from(first.is_ok()) + usize::from(second.is_ok());
    let conflict_count = usize::from(matches!(
        first,
        Err(ConfigPublishError::RevisionConflict { .. })
    )) + usize::from(matches!(
        second,
        Err(ConfigPublishError::RevisionConflict { .. })
    ));
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(success_count, 1);
    assert_eq!(conflict_count, 1);
    assert_eq!(stored.revision().get(), 2);
    assert_eq!(stored.proxies().profiles().len(), 2);
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 1);
}

#[tokio::test]
async fn operator_publish_rebases_only_across_automatic_oauth_refreshes() {
    let context = TestContext::new().await;
    let account_id = OAuthAccountId::new();
    let before_activation = context.snapshots.load().revision();
    let activated = context
        .publisher
        .activate_oauth_account(
            account_id,
            ProviderKind::Codex,
            oauth_account_draft("Refreshable OAuth"),
            OAuthProxySelection::Global,
            None,
            None,
            Vec::new(),
            oauth_document(ProviderKind::Codex, "initial-token", None),
        )
        .await
        .expect("activate OAuth account");

    let conflict = context
        .publisher
        .create_proxy(
            before_activation,
            ProxyProfileId::new(),
            proxy_draft("Stale Operator"),
        )
        .await
        .expect_err("OAuth activation is an operator publication");
    assert!(matches!(
        conflict,
        ConfigPublishError::RevisionConflict { .. }
    ));

    let (refreshed, changed) = context
        .publisher
        .refresh_oauth_accounts(
            vec![OAuthAccountRefresh::new(
                account_id,
                1,
                None,
                Some(1_900_000_000),
                oauth_document(ProviderKind::Codex, "refreshed-token", None),
            )],
            (),
        )
        .await
        .expect("refresh OAuth account batch");
    assert!(changed);
    assert!(refreshed.revision() > activated.revision());

    let endpoint_id = ProviderEndpointId::new();
    let published = context
        .publisher
        .create_provider_endpoint(activated.revision(), endpoint_id, codex_endpoint_draft())
        .await
        .expect("operator publish rebases across automatic refresh");

    assert!(published.revision() > refreshed.revision());
    assert!(published.provider_endpoints().get(endpoint_id).is_some());
}

#[tokio::test]
async fn concurrent_oauth_activations_each_publish_the_latest_revision() {
    let context = TestContext::new().await;
    let first_id = OAuthAccountId::new();
    let second_id = OAuthAccountId::new();

    let (first, second) = tokio::join!(
        context.publisher.activate_oauth_account(
            first_id,
            ProviderKind::Codex,
            oauth_account_draft("First OAuth"),
            OAuthProxySelection::Global,
            None,
            None,
            Vec::new(),
            oauth_document(ProviderKind::Codex, "first-token", None),
        ),
        context.publisher.activate_oauth_account(
            second_id,
            ProviderKind::Claude,
            oauth_account_draft("Second OAuth"),
            OAuthProxySelection::Global,
            Some("person@example.com".to_owned()),
            Some(1_800_000_000),
            Vec::new(),
            oauth_document(
                ProviderKind::Claude,
                "second-token",
                Some("person@example.com"),
            ),
        ),
    );
    let mut revisions = [
        first.expect("first activation").revision().get(),
        second.expect("second activation").revision().get(),
    ];
    revisions.sort_unstable();
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert_eq!(revisions, [2, 3]);
    assert_eq!(stored.revision().get(), 3);
    assert_eq!(stored.oauth_accounts().accounts().len(), 2);
    assert_eq!(context.snapshots.load().revision(), stored.revision());
    assert_eq!(context.runtime.scheduler_epoch(), 2);
}

struct TestContext {
    directory: TempDir,
    repository: Arc<SqliteStore>,
    snapshots: Arc<SnapshotStore>,
    runtime: Arc<RuntimeRegistry>,
    publisher: ConfigPublisher,
}

impl TestContext {
    async fn new() -> Self {
        let directory = tempdir().expect("temporary directory");
        let repository = Arc::new(
            SqliteStore::connect(&directory.path().join("config.sqlite3"))
                .await
                .expect("repository"),
        );
        let initial = repository
            .load_configuration()
            .await
            .expect("initial configuration");
        let runtime = Arc::new(RuntimeRegistry::new());
        let capabilities = crate::test_support::configuration_capabilities();
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(initial, runtime.as_ref(), capabilities.provider_registry())
                .expect("initial snapshot"),
        ));
        let publisher = ConfigPublisher::new(
            Arc::clone(&repository),
            Arc::clone(&snapshots),
            Arc::clone(&runtime),
            capabilities,
        )
        .expect("configuration publisher");

        Self {
            directory,
            repository,
            snapshots,
            runtime,
            publisher,
        }
    }
}

fn proxy_draft(name: &str) -> ProxyDraft {
    let address = ProxyAddress::new("proxy.example.com", 1080).expect("address");
    ProxyDraft::new(name, ProxyKind::Socks5, address, true).expect("draft")
}

fn codex_endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex Primary",
        ProviderKind::Codex,
        "https://api.example.com",
        ProtocolDialect::OpenAiResponses,
        None,
        true,
    )
    .expect("endpoint draft")
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Primary",
        CredentialKind::ApiKey,
        ProxyProfileId::DIRECT,
        None,
        true,
    )
    .expect("credential draft")
}

fn oauth_account_draft(label: &str) -> OAuthAccountDraft {
    OAuthAccountDraft::new(label, None, true).expect("OAuth account draft")
}

fn oauth_document(
    provider: ProviderKind,
    access_token: &str,
    email: Option<&str>,
) -> OAuthAccountDocument {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "access_token": access_token,
        "refresh_token": null,
        "id_token": null,
        "account_id": null,
        "email": email,
    }))
    .expect("OAuth document JSON")
    .into();
    OAuthAccountDocument::new(provider, bytes).expect("OAuth account document")
}
