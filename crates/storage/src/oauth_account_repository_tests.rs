use any2api_domain::{
    ConfigRevision, MaxConcurrency, OAuthAccountDraft, OAuthAccountId, ProviderKind,
};
use tempfile::tempdir;

use crate::{
    api::{ConfigurationRepository, OAuthAccountDocument, OAuthAccountRepository, SqliteStore},
    error::StorageError,
};

#[tokio::test]
async fn oauth_account_lifecycle_persists_plaintext_json_and_versions() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let account_id = OAuthAccountId::new();

    let created = store
        .create_oauth_account(
            ConfigRevision::INITIAL,
            account_id,
            ProviderKind::Codex,
            draft("Primary", 1, true),
            Some("owner@example.com".into()),
            Some(100),
            vec!["gpt-b".into(), "gpt-a".into()],
            document(ProviderKind::Codex, "first-access"),
        )
        .await
        .expect("create account");
    let account = created.oauth_accounts().get(account_id).expect("account");

    assert_eq!(created.revision().get(), 2);
    assert_eq!(account.token_version(), 1);
    assert_eq!(account.account_generation(), 1);
    assert_eq!(account.config_version(), 1);
    assert_eq!(
        account.proxy_profile_id(),
        any2api_domain::ProxyProfileId::DIRECT
    );
    assert_eq!(account.models()[0].as_str(), "gpt-a");
    assert_eq!(
        created
            .oauth_account_materials()
            .get(account_id)
            .expect("material")
            .document()
            .expose_for_test(),
        document_bytes(ProviderKind::Codex, "first-access").as_slice()
    );
    assert!(!format!("{created:?}").contains("first-access"));

    let no_op = store
        .update_oauth_account(created.revision(), account_id, 1, draft("Primary", 1, true))
        .await
        .expect("no-op update");
    assert_eq!(no_op.revision(), created.revision());

    let disabled = store
        .update_oauth_account(no_op.revision(), account_id, 1, draft("Primary", 2, false))
        .await
        .expect("disable account");
    let disabled_account = disabled.oauth_accounts().get(account_id).expect("disabled");
    assert_eq!(disabled_account.config_version(), 2);
    assert_eq!(disabled_account.account_generation(), 1);

    let enabled = store
        .update_oauth_account(
            disabled.revision(),
            account_id,
            2,
            draft("Primary", 2, true),
        )
        .await
        .expect("enable account");
    let with_models = store
        .set_oauth_account_models(enabled.revision(), account_id, 3, vec!["gpt-c".into()])
        .await
        .expect("replace models");
    let refreshed = store
        .refresh_oauth_account(
            with_models.revision(),
            account_id,
            1,
            Some("new@example.com".into()),
            Some(200),
            document(ProviderKind::Codex, "second-access"),
        )
        .await
        .expect("refresh account");
    let refreshed_account = refreshed
        .oauth_accounts()
        .get(account_id)
        .expect("refreshed account");
    assert_eq!(refreshed_account.token_version(), 2);
    assert_eq!(refreshed_account.account_generation(), 3);
    assert_eq!(refreshed_account.config_version(), 4);
    assert_eq!(
        refreshed_account.safe_account_email(),
        Some("new@example.com")
    );
    assert_eq!(refreshed_account.models()[0].as_str(), "gpt-c");
    assert_eq!(
        refreshed
            .oauth_account_materials()
            .get(account_id)
            .expect("refreshed material")
            .document()
            .expose_for_test(),
        document_bytes(ProviderKind::Codex, "second-access").as_slice()
    );

    let stale = store
        .refresh_oauth_account(
            refreshed.revision(),
            account_id,
            1,
            None,
            Some(300),
            document(ProviderKind::Codex, "stale-access"),
        )
        .await
        .expect_err("stale refresh must fail");
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
    assert_eq!(restored.revision(), refreshed.revision());
    assert_eq!(
        restored
            .oauth_account_materials()
            .get(account_id)
            .expect("restored material")
            .document()
            .expose_for_test(),
        document_bytes(ProviderKind::Codex, "second-access").as_slice()
    );

    let deleted = reopened
        .delete_oauth_account(restored.revision(), account_id, 4)
        .await
        .expect("delete account");
    assert!(deleted.oauth_accounts().get(account_id).is_none());
}

#[tokio::test]
async fn oauth_account_labels_are_unique_only_within_provider() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let codex = store
        .create_oauth_account(
            ConfigRevision::INITIAL,
            OAuthAccountId::new(),
            ProviderKind::Codex,
            draft("Primary", 1, true),
            None,
            None,
            vec!["gpt".into()],
            document(ProviderKind::Codex, "codex-access"),
        )
        .await
        .expect("Codex account");
    let claude = store
        .create_oauth_account(
            codex.revision(),
            OAuthAccountId::new(),
            ProviderKind::Claude,
            draft("Primary", 1, true),
            None,
            None,
            vec!["claude".into()],
            document(ProviderKind::Claude, "claude-access"),
        )
        .await
        .expect("Claude account");

    let error = store
        .create_oauth_account(
            claude.revision(),
            OAuthAccountId::new(),
            ProviderKind::Codex,
            draft("Primary", 1, true),
            None,
            None,
            vec!["gpt".into()],
            document(ProviderKind::Codex, "duplicate-access"),
        )
        .await
        .expect_err("duplicate label must fail");
    assert!(matches!(error, StorageError::OAuthAccountLabelConflict));
}

#[tokio::test]
async fn corrupt_oauth_json_fails_closed_without_exposing_token_data() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let account_id = OAuthAccountId::new();
    store
        .create_oauth_account(
            ConfigRevision::INITIAL,
            account_id,
            ProviderKind::Codex,
            draft("Primary", 1, true),
            None,
            None,
            vec!["gpt".into()],
            document(ProviderKind::Codex, "secret-access"),
        )
        .await
        .expect("account");
    sqlx::query("UPDATE oauth_accounts SET oauth_json = ? WHERE id = ?")
        .bind(b"{broken".as_slice())
        .bind(account_id.to_string())
        .execute(store.pool())
        .await
        .expect("corrupt OAuth JSON");

    let error = store
        .load_configuration()
        .await
        .expect_err("corrupt OAuth JSON must fail");
    assert!(matches!(error, StorageError::CorruptConfiguration));
    assert!(!format!("{error:?}").contains("secret-access"));
}

fn draft(label: &str, max_concurrency: u32, enabled: bool) -> OAuthAccountDraft {
    OAuthAccountDraft::new(
        label,
        MaxConcurrency::new(max_concurrency).expect("max concurrency"),
        enabled,
    )
    .expect("account draft")
}

fn document(provider: ProviderKind, access_token: &str) -> OAuthAccountDocument {
    OAuthAccountDocument::new(provider, document_bytes(provider, access_token).into())
        .expect("OAuth document")
}

fn document_bytes(provider: ProviderKind, access_token: &str) -> Vec<u8> {
    let provider = match provider {
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
    };
    format!(
        r#"{{"access_token":"{access_token}","refresh_token":"refresh-secret","type":"{provider}"}}"#
    )
    .into_bytes()
}
