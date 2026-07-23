use std::{sync::Arc, time::Instant};

use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId,
};
use any2api_provider::api::OAuthTokenMaterial;
use any2api_provider::{CodexDriver, ProviderRegistry};
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    TransportManager, TransportProxy, TransportRequest, TransportResponse,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, StatusCode, header::AUTHORIZATION};
use tempfile::tempdir;

use crate::{
    provider_oauth::{
        ProviderOAuthError, ProviderOAuthService, callback,
        session::{OAuthSession, OAuthSessionStore, SESSION_TTL_SECONDS},
    },
    provider_oauth2_secret::ProviderOAuth2Secret,
    published_snapshot::{PublishedSnapshot, SnapshotStore},
    publisher::ConfigPublisher,
    registry::RuntimeRegistry,
};

#[test]
fn oauth_sessions_enforce_state_expiry_and_single_use() {
    let now = Instant::now();
    let prepared = OAuthSession::prepare(
        ProviderKind::Codex,
        ProviderEndpointId::new(),
        1,
        ProxyProfileId::DIRECT,
        1,
        oauth_draft(),
        "http://localhost:1455/auth/callback",
        now,
    )
    .expect("prepared session");
    let callback_url = format!(
        "http://localhost:1455/auth/callback?code=one-time-code&state={}",
        prepared.state
    );
    assert_eq!(
        callback::parse(
            &callback_url,
            "http://localhost:1455/auth/callback",
            &prepared.state,
        )
        .expect("callback")
        .code,
        "one-time-code"
    );
    assert!(matches!(
        callback::parse(
            "http://localhost:1455/auth/callback?code=one-time-code&state=wrong",
            "http://localhost:1455/auth/callback",
            &prepared.state,
        ),
        Err(ProviderOAuthError::StateMismatch)
    ));

    let mut store = OAuthSessionStore::default();
    let session_id = prepared.id.clone();
    store
        .insert(prepared.id, prepared.session, now)
        .expect("insert session");
    store.take(&session_id, now).expect("first use");
    assert!(matches!(
        store.take(&session_id, now),
        Err(ProviderOAuthError::InvalidSession)
    ));

    let expired = OAuthSession::prepare(
        ProviderKind::Codex,
        ProviderEndpointId::new(),
        1,
        ProxyProfileId::DIRECT,
        1,
        oauth_draft(),
        "http://localhost:1455/auth/callback",
        now,
    )
    .expect("expired session");
    let expired_id = expired.id.clone();
    store
        .insert(expired.id, expired.session, now)
        .expect("insert expiring session");
    assert!(matches!(
        store.take(
            &expired_id,
            now + std::time::Duration::from_secs(SESSION_TTL_SECONDS + 1),
        ),
        Err(ProviderOAuthError::SessionExpired)
    ));
}

#[tokio::test]
async fn due_oauth_refresh_rotates_secret_without_clearing_models() {
    let directory = tempdir().expect("temporary directory");
    let storage = Arc::new(
        SqliteStore::connect(&directory.path().join("any2api.sqlite3"))
            .await
            .expect("storage"),
    );
    let configuration = storage.load_configuration().await.expect("configuration");
    let runtime = Arc::new(RuntimeRegistry::new(configuration.settings().scheduler()));
    let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
        configuration,
        runtime.as_ref(),
    )));
    let publisher = Arc::new(ConfigPublisher::new(
        Arc::clone(&storage),
        Arc::clone(&snapshots),
        Arc::clone(&runtime),
    ));
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    publisher
        .create_provider_endpoint(
            ConfigRevision::INITIAL,
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex OAuth",
                ProviderKind::Codex,
                "https://chatgpt.com/backend-api/codex",
                ProtocolDialect::OpenAiResponses,
                true,
            )
            .expect("endpoint draft"),
        )
        .await
        .expect("endpoint");
    let initial = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        "old-access".to_owned(),
        Some("old-refresh".to_owned()),
        None,
        Some(1),
        None,
        None,
        None,
        None,
    )
    .expect("initial token");
    let created = publisher
        .create_provider_oauth_credential(
            credential_id,
            endpoint_id,
            1,
            oauth_draft(),
            ProviderOAuth2Secret::from_token(&initial).expect("OAuth secret"),
        )
        .await
        .expect("OAuth credential");
    publisher
        .set_provider_credential_models(
            created.revision(),
            credential_id,
            1,
            vec!["gpt-5.1-codex".to_owned()],
        )
        .await
        .expect("models");

    let mut registry = ProviderRegistry::new();
    registry
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    let registry = Arc::new(registry);
    let transport = Arc::new(RefreshTransport::default());
    let service = ProviderOAuthService::new(
        Arc::clone(&registry),
        transport.clone(),
        Arc::clone(&snapshots),
        Arc::clone(&publisher),
    );
    service
        .refresh_credential_for_test(credential_id)
        .await
        .expect("refresh credential");

    let snapshot = snapshots.load();
    let credential = snapshot
        .provider_credentials()
        .get(credential_id)
        .expect("refreshed credential");
    assert_eq!(credential.secret_version(), 2);
    assert_eq!(credential.credential_generation(), 2);
    assert_eq!(credential.models()[0].as_str(), "gpt-5.1-codex");
    let driver = registry.get(ProviderKind::Codex).expect("driver");
    let permit = snapshot
        .credential_runtime(credential_id)
        .expect("runtime")
        .try_acquire()
        .expect("permit");
    let headers = permit
        .provider_credential_headers(driver.as_ref())
        .expect("headers");
    assert_eq!(headers.headers[AUTHORIZATION], "Bearer new-access");
    assert!(
        transport
            .request_body
            .lock()
            .expect("request body")
            .contains("old-refresh")
    );
}

fn oauth_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Personal OAuth",
        CredentialKind::OAuth2,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(2).expect("max concurrency"),
        true,
    )
    .expect("OAuth draft")
}

#[derive(Default)]
struct RefreshTransport {
    request_body: std::sync::Mutex<String>,
}

#[async_trait]
impl TransportManager for RefreshTransport {
    async fn execute(
        &self,
        _proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        *self.request_body.lock().expect("request body") =
            String::from_utf8(request.body.to_vec()).expect("UTF-8 request body");
        Ok(TransportResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::iter([Ok(Bytes::from_static(
                br#"{"access_token":"new-access","refresh_token":"new-refresh","expires_in":3600}"#,
            ))])),
            read_failure_scope: any2api_transport::api::TransportFailureScope::Endpoint,
        })
    }
}
