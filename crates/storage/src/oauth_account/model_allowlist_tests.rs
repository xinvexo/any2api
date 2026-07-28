use any2api_domain::{
    ConfigRevision, OAuthAccountDraft, OAuthAccountId, ProviderKind, SettingKey, SettingValue,
};
use tempfile::tempdir;

use crate::api::{
    ConfigurationRepository, OAuthAccountDocument, OAuthAccountRepository, SettingRepository,
    SqliteStore,
};

#[tokio::test]
async fn model_allowlist_tracks_oauth_sources_without_treating_disabled_as_removed() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let account_id = OAuthAccountId::new();
    let created = store
        .create_oauth_account(
            ConfigRevision::INITIAL,
            account_id,
            ProviderKind::Codex,
            draft(true),
            None,
            None,
            vec!["gpt-a".into(), "gpt-b".into()],
            document(),
        )
        .await
        .expect("OAuth account");
    let allowed = store
        .set_setting_override(
            created.revision(),
            SettingKey::ModelsAllowed,
            SettingValue::StringList(vec!["gpt-a".into(), "gpt-b".into()]),
        )
        .await
        .expect("model allowlist");

    let disabled = store
        .update_oauth_account(allowed.revision(), account_id, 1, draft(false))
        .await
        .expect("disable account");
    assert_eq!(
        disabled
            .settings()
            .override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::StringList(vec![
            "gpt-a".into(),
            "gpt-b".into(),
        ]))
    );

    let reduced = store
        .set_oauth_account_models(disabled.revision(), account_id, 2, vec!["gpt-b".into()])
        .await
        .expect("remove one OAuth model source");
    assert_eq!(
        reduced.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::StringList(vec!["gpt-b".into()]))
    );

    let deleted = store
        .delete_oauth_account(reduced.revision(), account_id, 3)
        .await
        .expect("delete OAuth account");
    assert_eq!(
        deleted.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::StringList(Vec::new()))
    );

    drop(store);
    let restored = SqliteStore::connect(&database)
        .await
        .expect("reopen")
        .load_configuration()
        .await
        .expect("configuration");
    assert_eq!(restored.settings(), deleted.settings());
}

fn draft(enabled: bool) -> OAuthAccountDraft {
    OAuthAccountDraft::new("Primary", None, enabled).expect("OAuth draft")
}

fn document() -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        ProviderKind::Codex,
        br#"{"access_token":"access","refresh_token":"refresh","type":"codex"}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}
