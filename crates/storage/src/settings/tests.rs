use any2api_domain::{
    ConfigRevision, RateLimitMode, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigurationMutation, ConfigurationRepository, commit_configuration},
    error::StorageError,
    sqlite::SqliteStore,
};

#[tokio::test]
async fn scheduler_overrides_persist_and_reset_to_compiled_defaults() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("settings.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let initial = store.load_configuration().await.expect("initial settings");

    assert_eq!(
        initial.settings().scheduler().on_rate_limited(),
        RateLimitMode::Wait
    );
    assert_eq!(
        initial
            .settings()
            .override_value(SettingKey::SchedulerOnRateLimited),
        None
    );

    let updated = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerOnRateLimited,
                value: SettingValue::RateLimitMode(RateLimitMode::Reject),
            }],
        },
    )
    .await
    .expect("override setting");
    assert_eq!(updated.revision().get(), 2);
    assert_eq!(
        updated.settings().scheduler().on_rate_limited(),
        RateLimitMode::Reject
    );

    let no_op = commit_configuration(
        &store,
        updated.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerOnRateLimited,
                value: SettingValue::RateLimitMode(RateLimitMode::Reject),
            }],
        },
    )
    .await
    .expect("same override is a no-op");
    assert_eq!(no_op.revision(), updated.revision());

    drop(store);
    let reopened = SqliteStore::connect(&database)
        .await
        .expect("reopened store");
    let persisted = reopened
        .load_configuration()
        .await
        .expect("persisted settings");
    assert_eq!(
        persisted.settings().scheduler().on_rate_limited(),
        RateLimitMode::Reject
    );

    let reset = commit_configuration(
        &reopened,
        persisted.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Reset {
                key: SettingKey::SchedulerOnRateLimited,
            }],
        },
    )
    .await
    .expect("reset setting");
    assert_eq!(reset.revision().get(), 3);
    assert_eq!(
        reset.settings().scheduler().on_rate_limited(),
        RateLimitMode::Wait
    );
    assert_eq!(
        reset
            .settings()
            .override_value(SettingKey::SchedulerOnRateLimited),
        None
    );
}

#[tokio::test]
async fn explicit_override_equal_to_default_is_preserved() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("settings.sqlite3"))
        .await
        .expect("store");

    let updated = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerFallbackOnRateLimit,
                value: SettingValue::Boolean(false),
            }],
        },
    )
    .await
    .expect("explicit default override");

    assert_eq!(updated.revision().get(), 2);
    assert_eq!(
        updated
            .settings()
            .override_value(SettingKey::SchedulerFallbackOnRateLimit),
        Some(SettingValue::Boolean(false))
    );
}

#[tokio::test]
async fn setting_batch_validates_and_commits_as_one_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("settings.sqlite3"))
        .await
        .expect("store");

    let updated = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![
                SettingOverrideChange::Set {
                    key: SettingKey::OAuthRefreshScanInterval,
                    value: SettingValue::DurationSecs(600),
                },
                SettingOverrideChange::Set {
                    key: SettingKey::OAuthRefreshLeadTime,
                    value: SettingValue::DurationSecs(900),
                },
            ],
        },
    )
    .await
    .expect("atomic setting batch");
    assert_eq!(updated.revision().get(), 2);
    assert_eq!(updated.settings().oauth().refresh_scan_interval_secs(), 600);
    assert_eq!(updated.settings().oauth().refresh_lead_time_secs(), 900);

    let invalid = commit_configuration(
        &store,
        updated.revision(),
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![
                SettingOverrideChange::Set {
                    key: SettingKey::OAuthRefreshScanInterval,
                    value: SettingValue::DurationSecs(1_000),
                },
                SettingOverrideChange::Set {
                    key: SettingKey::OAuthRefreshLeadTime,
                    value: SettingValue::DurationSecs(500),
                },
            ],
        },
    )
    .await
    .expect_err("invalid batch");
    assert!(matches!(invalid, StorageError::SettingsValidation(_)));
    let unchanged = store
        .load_configuration()
        .await
        .expect("unchanged settings");
    assert_eq!(unchanged.revision(), updated.revision());
    assert_eq!(unchanged.settings(), updated.settings());
}

#[tokio::test]
async fn stale_revision_and_corrupt_rows_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("settings.sqlite3"))
        .await
        .expect("store");
    let updated = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerMaxWaitingRequests,
                value: SettingValue::Integer(64),
            }],
        },
    )
    .await
    .expect("first override");

    let conflict = commit_configuration(
        &store,
        ConfigRevision::INITIAL,
        ConfigurationMutation::ApplySettingChanges {
            changes: vec![SettingOverrideChange::Set {
                key: SettingKey::SchedulerMaxWaitingRequests,
                value: SettingValue::Integer(32),
            }],
        },
    )
    .await
    .expect_err("stale revision");
    assert!(matches!(conflict, StorageError::RevisionConflict { .. }));
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("unchanged settings")
            .revision(),
        updated.revision()
    );

    sqlx::query("INSERT INTO setting_overrides (key, value_json) VALUES (?, ?)")
        .bind("scheduler.unknown")
        .bind("true")
        .execute(store.pool())
        .await
        .expect("corrupt row");
    assert!(matches!(
        store.load_configuration().await,
        Err(StorageError::CorruptConfiguration)
    ));
}
