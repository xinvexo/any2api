use any2api_domain::{ConfigRevision, OAuthAccountId, ProviderKind};
use tempfile::tempdir;

use super::{create, document};
use crate::{
    api::OAuthAccountRefresh,
    configuration::{ConfigurationMutation, ConfigurationRepository, commit_configuration},
    error::StorageError,
    sqlite::SqliteStore,
};

#[tokio::test]
async fn oauth_account_batch_is_one_transaction_and_one_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let codex_id = OAuthAccountId::new();
    let claude_id = OAuthAccountId::new();

    let created = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateOAuthAccounts {
            accounts: vec![
                create(codex_id, ProviderKind::Codex, "Codex Imported"),
                create(claude_id, ProviderKind::Claude, "Claude Imported"),
            ],
        },
    )
    .await
    .expect("batch create");

    assert_eq!(created.revision().get(), 2);
    assert!(created.oauth_accounts().get(codex_id).is_some());
    assert!(created.oauth_accounts().get(claude_id).is_some());
    let persisted_revision =
        sqlx::query_scalar::<_, i64>("SELECT revision FROM config_state WHERE singleton_id = 1")
            .fetch_one(store.pool())
            .await
            .expect("revision");
    assert_eq!(persisted_revision, 2);
}

#[tokio::test]
async fn invalid_oauth_account_batch_rolls_back_every_account() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");

    let error = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateOAuthAccounts {
            accounts: vec![
                create(OAuthAccountId::new(), ProviderKind::Codex, "Duplicate"),
                create(OAuthAccountId::new(), ProviderKind::Codex, "Duplicate"),
            ],
        },
    )
    .await
    .expect_err("duplicate batch label");
    assert!(matches!(error, StorageError::OAuthAccountLabelConflict));

    let restored = store.load_configuration().await.expect("configuration");
    assert_eq!(restored.revision(), ConfigRevision::INITIAL);
    assert!(restored.oauth_accounts().accounts().is_empty());
    let row_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM oauth_accounts")
        .fetch_one(store.pool())
        .await
        .expect("row count");
    assert_eq!(row_count, 0);
}

#[tokio::test]
async fn refresh_batch_skips_stale_accounts_and_advances_one_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("store");
    let stale_id = OAuthAccountId::new();
    let fresh_id = OAuthAccountId::new();
    let created = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::CreateOAuthAccounts {
            accounts: vec![
                create(stale_id, ProviderKind::Codex, "Stale"),
                create(fresh_id, ProviderKind::Claude, "Fresh"),
            ],
        },
    )
    .await
    .expect("accounts");
    let independently_refreshed = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::RefreshOAuthAccount {
            id: stale_id,
            expected_token_version: 1,
            safe_account_email: None,
            expires_at: Some(200),
            document: document(ProviderKind::Codex, "newer-access"),
        },
    )
    .await
    .expect("independent refresh");

    let batched = commit_configuration(
        &store,
        independently_refreshed.revision(),
        ConfigurationMutation::RefreshOAuthAccounts {
            refreshes: vec![
                OAuthAccountRefresh::new(
                    stale_id,
                    1,
                    None,
                    Some(300),
                    document(ProviderKind::Codex, "stale-access"),
                ),
                OAuthAccountRefresh::new(
                    fresh_id,
                    1,
                    Some("fresh@example.com".into()),
                    Some(400),
                    document(ProviderKind::Claude, "fresh-access"),
                ),
            ],
        },
    )
    .await
    .expect("batch refresh");

    assert_eq!(
        batched.revision().get(),
        independently_refreshed.revision().get() + 1
    );
    assert_eq!(
        batched
            .oauth_accounts()
            .get(stale_id)
            .expect("stale account")
            .token_version(),
        2
    );
    let fresh = batched
        .oauth_accounts()
        .get(fresh_id)
        .expect("fresh account");
    assert_eq!(fresh.token_version(), 2);
    assert_eq!(fresh.safe_account_email(), Some("fresh@example.com"));

    let all_stale = commit_configuration(
        &store,
        batched.revision(),
        ConfigurationMutation::RefreshOAuthAccounts {
            refreshes: vec![OAuthAccountRefresh::new(
                fresh_id,
                1,
                None,
                Some(500),
                document(ProviderKind::Claude, "also-stale"),
            )],
        },
    )
    .await
    .expect("all-stale batch");
    assert_eq!(all_stale.revision(), batched.revision());
}
