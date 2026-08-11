use std::sync::Arc;

use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::{ClaudeDriver, CodexDriver, GrokDriver, api::ProviderRegistry};
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, OAuthAccountDocument, SqliteStore,
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
    pub(super) storage: Arc<SqliteStore>,
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

    pub(super) async fn new_blocking_refresh(available_count: u32) -> Self {
        Self::with_transport(Arc::new(QuotaTransport::new_blocking_usage(
            available_count,
        )))
        .await
    }

    pub(super) async fn new_without_refresh_token() -> Self {
        Self::with_account(
            Arc::new(QuotaTransport::new(
                0,
                AuthenticationMode::AlwaysReject,
                false,
            )),
            ProviderKind::Codex,
            "Codex OAuth",
            Some("person@example.com".into()),
            vec!["gpt-5.5".into()],
            codex_oauth_document_without_refresh_token(),
        )
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
            vec!["claude-sonnet-4-5-20250929".into()],
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
        let configured = crate::test_support::commit_configuration(
            storage.as_ref(),
            initial.revision(),
            ConfigurationMutation::CreateOAuthAccount {
                id: account_id,
                provider_kind: provider,
                draft: OAuthAccountDraft::new(label, None, true).expect("OAuth draft"),
                safe_account_email: email,
                expires_at: None,
                models,
                document,
            },
        )
        .await
        .expect("OAuth account");
        let providers = providers();
        let runtime = Arc::new(RuntimeRegistry::new());
        let snapshots = Arc::new(SnapshotStore::new(
            PublishedSnapshot::new(configured, runtime.as_ref(), providers.as_ref())
                .expect("initial snapshot"),
        ));
        let publisher = Arc::new(
            ConfigPublisher::new(
                Arc::clone(&storage),
                Arc::clone(&snapshots),
                Arc::clone(&runtime),
                crate::test_support::configuration_capabilities(),
            )
            .expect("publisher"),
        );
        let service = Arc::new(OAuthService::new_for_test(
            providers,
            Arc::clone(&transport) as Arc<dyn TransportManager>,
            publisher,
            Arc::clone(&storage),
        ));
        Self {
            _directory: directory,
            storage,
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
        br#"{"access_token":"old-access","refresh_token":"old-refresh","id_token":null,"account_id":"account-123","email":null}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}

fn codex_oauth_document_without_refresh_token() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        br#"{"access_token":"old-access","refresh_token":null,"id_token":null,"account_id":"account-123","email":null}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}

fn grok_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Grok,
        br#"{"access_token":"grok-access","refresh_token":"grok-refresh","id_token":null,"account_id":"grok-subject","email":null}"#
            .to_vec()
            .into(),
    )
    .expect("Grok OAuth document")
}

fn claude_oauth_document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Claude,
        br#"{"access_token":"old-access","refresh_token":"old-refresh","id_token":null,"account_id":null,"email":"claude@example.com"}"#
            .to_vec()
            .into(),
    )
    .expect("Claude OAuth document")
}
