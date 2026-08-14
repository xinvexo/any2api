use any2api_domain::{
    ConfigRevision, ModelAccess, OAuthAccountDraft, OAuthAccountId, OAuthProxySelection,
    ProviderKind, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigurationMutation, ConfigurationRepository, commit_configuration},
    oauth_account::OAuthAccountDocument,
    sqlite::SqliteStore,
};

#[tokio::test]
async fn model_allowlist_tracks_oauth_sources_without_treating_disabled_as_removed() {
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
            draft: draft(true),
            proxy_selection: OAuthProxySelection::Global,
            safe_account_email: None,
            expires_at: None,
            models: vec!["gpt-a".into(), "gpt-b".into()],
            document: document(),
        },
    )
    .await
    .expect("OAuth account");
    let allowed = commit_configuration(
        &store,
        created.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::ModelsAllowed,
                value: SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
                    "gpt-a".into(),
                    "gpt-b".into(),
                ])),
            }],
        },
    )
    .await
    .expect("model allowlist");

    let disabled = commit_configuration(
        &store,
        allowed.revision(),
        ConfigurationMutation::UpdateOAuthAccount {
            id: account_id,
            expected_config_version: 1,
            draft: draft(false),
            proxy_selection: OAuthProxySelection::Global,
        },
    )
    .await
    .expect("disable account");
    assert_eq!(
        disabled
            .settings()
            .override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
            "gpt-a".into(),
            "gpt-b".into(),
        ])))
    );

    let reduced = commit_configuration(
        &store,
        disabled.revision(),
        ConfigurationMutation::SetOAuthAccountModels {
            id: account_id,
            expected_config_version: 2,
            models: vec!["gpt-b".into()],
        },
    )
    .await
    .expect("remove one OAuth model source");
    assert_eq!(
        reduced.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(ModelAccess::Allowlist(vec![
            "gpt-b".into()
        ])))
    );

    let deleted = commit_configuration(
        &store,
        reduced.revision(),
        ConfigurationMutation::DeleteOAuthAccount {
            id: account_id,
            expected_config_version: 3,
        },
    )
    .await
    .expect("delete OAuth account");
    assert_eq!(
        deleted.settings().override_value(SettingKey::ModelsAllowed),
        Some(SettingValue::ModelAccess(
            ModelAccess::Allowlist(Vec::new())
        ))
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
        br#"{"access_token":"access","refresh_token":"refresh","id_token":null,"account_id":null,"email":null}"#
            .to_vec()
            .into(),
    )
    .expect("OAuth document")
}
