use any2api_domain::{
    ConfigRevision, OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind,
    ProxyProfileId, RequestsPerMinute,
};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigurationMutation, ConfigurationRepository, commit_configuration},
    error::StorageError,
    oauth_account::OAuthAccountDocument,
    sqlite::SqliteStore,
};

#[tokio::test]
async fn reauthorization_updates_proxy_persists_models_and_uses_token_cas() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let account_id = OAuthAccountId::new();
    let created = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateOAuthAccount {
            id: account_id,
            provider_kind: ProviderKind::Codex,
            draft: draft("Local settings", Some(17), false),
            proxy_selection: OAuthProxySelection::Global,
            safe_account_email: Some("old@example.com".into()),
            expires_at: Some(100),
            models: vec!["keep-model".into(), "removed-model".into()],
            document: document("old-access"),
        },
    )
    .await
    .expect("create account");

    let reauthorized = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::ReauthorizeOAuthAccount {
            id: account_id,
            expected_token_version: 1,
            proxy_selection: OAuthProxySelection::Profile(ProxyProfileId::DIRECT),
            safe_account_email: Some("new@example.com".into()),
            expires_at: Some(200),
            models: vec!["keep-model".into()],
            document: document("new-access"),
        },
    )
    .await
    .expect("reauthorize account");
    assert_reauthorized(&reauthorized, account_id);

    let stale = commit_configuration(
        &store,
        reauthorized.revision(),
        ConfigurationMutation::ReauthorizeOAuthAccount {
            id: account_id,
            expected_token_version: 1,
            proxy_selection: OAuthProxySelection::Global,
            safe_account_email: None,
            expires_at: None,
            models: vec![],
            document: document("stale-access"),
        },
    )
    .await
    .expect_err("stale reauthorization must fail");
    assert!(matches!(
        stale,
        StorageError::OAuthAccountTokenVersionConflict {
            expected: 1,
            actual: 2
        }
    ));

    drop(store);
    let reopened = SqliteStore::connect(&database).await.expect("reopen store");
    let restored = reopened
        .load_configuration()
        .await
        .expect("restored configuration");
    assert_eq!(restored.revision(), reauthorized.revision());
    assert_reauthorized(&restored, account_id);
}

fn assert_reauthorized(
    configuration: &crate::configuration::StoredConfiguration,
    account_id: OAuthAccountId,
) {
    let account = configuration
        .oauth_accounts()
        .get(account_id)
        .expect("reauthorized account");
    assert_eq!(account.label(), "Local settings");
    assert_eq!(
        account.requests_per_minute().map(|value| value.get()),
        Some(17)
    );
    assert!(!account.enabled());
    assert_eq!(account.token_version(), 2);
    assert_eq!(account.account_generation(), 2);
    assert_eq!(account.config_version(), 2);
    assert_eq!(
        account.proxy_selection(),
        OAuthProxySelection::Profile(ProxyProfileId::DIRECT)
    );
    assert_eq!(account.safe_account_email(), Some("new@example.com"));
    assert_eq!(account.expires_at(), Some(200));
    assert_eq!(account.models().len(), 1);
    assert_eq!(account.models()[0].as_str(), "keep-model");
    assert_eq!(
        configuration
            .oauth_account_materials()
            .get(account_id)
            .expect("reauthorized material")
            .document()
            .expose_for_test(),
        document_bytes("new-access")
    );
}

fn draft(label: &str, requests_per_minute: Option<u32>, enabled: bool) -> OAuthAccountDraft {
    OAuthAccountDraft::new(
        label,
        requests_per_minute.map(|value| RequestsPerMinute::new(value).expect("valid RPM")),
        enabled,
    )
    .expect("account draft")
}

fn document(access_token: &str) -> OAuthAccountDocument {
    OAuthAccountDocument::new(ProviderKind::Codex, document_bytes(access_token).into())
        .expect("OAuth document")
}

fn document_bytes(access_token: &str) -> Vec<u8> {
    format!(
        r#"{{"access_token":"{access_token}","refresh_token":"refresh-secret","id_token":null,"account_id":null,"email":null}}"#
    )
    .into_bytes()
}
