use std::sync::Arc;

use any2api_domain::{OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::{
    ConfigurationMutation, ConfigurationRepository, OAuthAccountDocument, SqliteStore,
};
use any2api_transport::api::TransportManager;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

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
    pub(super) publisher: Arc<ConfigPublisher>,
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
                proxy_selection: OAuthProxySelection::Global,
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
            Arc::clone(&publisher),
            Arc::clone(&storage),
        ));
        Self {
            _directory: directory,
            storage,
            snapshots,
            runtime,
            publisher,
            service,
            transport,
            account_id,
        }
    }

    pub(super) async fn add_codex_account(&self, account_id: &str) -> OAuthAccountId {
        self.add_codex_account_with_document(account_id, codex_oauth_document_for(account_id))
            .await
    }

    pub(super) async fn add_codex_account_with_plan(
        &self,
        account_id: &str,
        plan: &str,
    ) -> OAuthAccountId {
        self.add_codex_account_with_document(
            account_id,
            codex_oauth_document_for_plan(account_id, plan),
        )
        .await
    }

    async fn add_codex_account_with_document(
        &self,
        account_id: &str,
        document: OAuthAccountDocument,
    ) -> OAuthAccountId {
        let id = OAuthAccountId::new();
        let snapshot = self
            .publisher
            .activate_oauth_account(
                id,
                ProviderKind::Codex,
                OAuthAccountDraft::new(format!("Codex {account_id}"), None, true)
                    .expect("OAuth draft"),
                OAuthProxySelection::Global,
                None,
                None,
                vec!["gpt-5.5".into()],
                document,
            )
            .await
            .expect("OAuth account");
        assert!(snapshot.oauth_accounts().get(id).is_some());
        id
    }
}

fn providers() -> Arc<ProviderRegistry> {
    let mut providers = ProviderRegistry::new();
    providers
        .register(Arc::new(crate::test_support::codex_driver()))
        .expect("Codex driver");
    providers
        .register(Arc::new(crate::test_support::grok_driver()))
        .expect("Grok driver");
    providers
        .register(Arc::new(crate::test_support::claude_driver()))
        .expect("Claude driver");
    Arc::new(providers)
}

fn codex_oauth_document() -> OAuthAccountDocument {
    let id_token = codex_id_token_for_plan("free");
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        serde_json::json!({
            "access_token": "old-access",
            "refresh_token": "old-refresh",
            "id_token": id_token,
            "account_id": "account-123",
            "email": null,
        })
        .to_string()
        .into_bytes()
        .into(),
    )
    .expect("OAuth document")
}

fn codex_oauth_document_for(account_id: &str) -> OAuthAccountDocument {
    codex_oauth_document_for_plan(account_id, "free")
}

fn codex_oauth_document_for_plan(account_id: &str, plan: &str) -> OAuthAccountDocument {
    let document = serde_json::json!({
        "access_token": format!("old-access-{account_id}"),
        "refresh_token": format!("old-refresh-{account_id}"),
        "id_token": codex_id_token_for_plan(plan),
        "account_id": account_id,
        "email": null,
    });
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        document.to_string().into_bytes().into(),
    )
    .expect("OAuth document")
}

fn codex_id_token_for_plan(plan: &str) -> String {
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": plan,
            },
        })
        .to_string(),
    );
    format!("header.{payload}.signature")
}

fn codex_oauth_document_without_refresh_token() -> OAuthAccountDocument {
    let id_token = codex_id_token_for_plan("free");
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        serde_json::json!({
            "access_token": "old-access",
            "refresh_token": null,
            "id_token": id_token,
            "account_id": "account-123",
            "email": null,
        })
        .to_string()
        .into_bytes()
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
