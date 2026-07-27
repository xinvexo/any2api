use std::sync::Arc;

use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::{ClaudeDriver, CodexDriver, GrokDriver, ProviderRegistry};
use any2api_storage::api::{
    ConfigurationRepository, OAuthAccountDocument, OAuthAccountRepository, SqliteStore,
};
use any2api_transport::api::TransportManager;

pub(super) use super::mock_transport::AuthenticationMode;
use super::mock_transport::QuotaTransport;
use crate::oauth::OAuthService;
use crate::{
    configuration::{ConfigPublisher, PublishedSnapshot, SnapshotStore},
    registry::RuntimeRegistry,
};

pub(super) struct QuotaTestContext {
    _directory: tempfile::TempDir,
    _storage: Arc<SqliteStore>,
    pub(super) snapshots: Arc<SnapshotStore>,
    pub(super) runtime: Arc<RuntimeRegistry>,
    pub(super) service: Arc<OAuthService>,
    pub(super) transport: Arc<QuotaTransport>,
    pub(super) account_id: OAuthAccountId,
}

impl QuotaTestContext {
    pub(super) async fn new(available_count: u32, authentication: AuthenticationMode) -> Self {
        Self::with_transport(Arc::new(QuotaTransport::new(
            available_count,
            authentication,
            false,
        )))
        .await
    }

    pub(super) async fn new_blocking_reset(available_count: u32) -> Self {
        Self::with_transport(Arc::new(QuotaTransport::new(
            available_count,
            AuthenticationMode::Accepted,
            true,
        )))
        .await
    }

    pub(super) async fn new_grok() -> Self {
        Self::new_grok_with_authentication(AuthenticationMode::Accepted).await
    }

    pub(super) async fn new_grok_with_authentication(authentication: AuthenticationMode) -> Self {
        Self::with_account(
            Arc::new(QuotaTransport::new(0, authentication, false)),
            ProviderKind::Grok,
            "Grok OAuth",
            None,
            vec!["grok-4.5".into()],
            grok_oauth_document(),
        )
        .await
    }

    pub(super) async fn new_grok_unified() -> Self {
        Self::with_account(
            Arc::new(QuotaTransport::new_grok_unified()),
            ProviderKind::Grok,
            "Grok OAuth",
            None,
            vec!["grok-4.5".into()],
            grok_oauth_document(),
        )
        .await
    }

    pub(super) async fn new_claude(authentication: AuthenticationMode) -> Self {
        Self::with_account(
            Arc::new(QuotaTransport::new(0, authentication, false)),
            ProviderKind::Claude,
            "Claude OAuth",
            Some("claude@example.com".into()),
            vec!["claude-sonnet-4-5".into()],
            claude_oauth_document(),
        )
        .await
    }

    async fn with_transport(transport: Arc<QuotaTransport>) -> Self {
        Self::with_account(
            transport,
            ProviderKind::Codex,
            "Codex OAuth",
            Some("person@example.com".into()),
            vec!["gpt-5.5".into()],
            codex_oauth_document(),
        )
        .await
    }

    async fn with_account(
        transport: Arc<QuotaTransport>,
        provider: ProviderKind,
        label: &str,
        email: Option<String>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory");
        let storage = Arc::new(
            SqliteStore::connect(&directory.path().join("oauth-quota.sqlite3"))
                .await
                .expect("storage"),
        );
        let initial = storage.load_configuration().await.expect("configuration");
        let account_id = OAuthAccountId::new();
        let configured = storage
            .create_oauth_account(
                initial.revision(),
                account_id,
                provider,
                OAuthAccountDraft::new(label, None, true).expect("OAuth draft"),
                email,
                None,
                models,
                document,
            )
            .await
            .expect("OAuth account");
        let providers = providers();
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(PublishedSnapshot::new(
            configured,
            runtime.as_ref(),
            providers.as_ref(),
        )));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                crate::test_support::configuration_capabilities(),
            )
            .expect("publisher"),
        );
        let service = Arc::new(OAuthService::new(
            providers,
            Arc::clone(&transport) as Arc<dyn TransportManager>,
            publisher,
        ));
        Self {
            _directory: directory,
            _storage: storage,
            snapshots,
            runtime,
            service,
            transport,
            account_id,
        }
    }
}

fn providers() -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex driver");
    providers
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok driver");
    providers
        .register(Arc::new(ClaudeDriver::new()))
        .expect("Claude driver");
    Arc::new(providers)
}

fn codex_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        br#"{"type":"codex","access_token":"old-access","refresh_token":"old-refresh","account_id":"account-123"}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}

fn grok_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Grok,
        br#"{"type":"grok","access_token":"grok-access","refresh_token":"grok-refresh","sub":"grok-subject"}"#
            .to_vec()
            .into(),
    )
    .expect("Grok OAuth document")
}

fn claude_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Claude,
        br#"{"type":"claude","access_token":"old-access","refresh_token":"old-refresh"}"#
            .to_vec()
            .into(),
    )
    .expect("Claude OAuth document")
}
