use any2api_domain::{
    ConfigRevision, RateLimitMode, SettingKey, SettingOverrideChange, SettingValue,
};
use tempfile::tempdir;

use crate::{
    configuration::{ConfigurationMutation, ConfigurationRepository},
    sqlite::SqliteStore,
};

#[tokio::test]
async fn candidate_stays_uncommitted_until_finish() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");

    let prepared = store
        .prepare_configuration(
            ConfigRevision::INITIAL,
            ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Set {
                    key: SettingKey::SchedulerOnRateLimited,
                    value: SettingValue::RateLimitMode(RateLimitMode::Reject),
                }],
            },
        )
        .await
        .expect("prepared candidate");

    assert!(prepared.changed());
    assert_eq!(prepared.candidate().revision().get(), 2);
    assert_eq!(
        prepared
            .candidate()
            .settings()
            .scheduler()
            .on_rate_limited(),
        RateLimitMode::Reject
    );
    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("concurrent committed view")
            .revision(),
        ConfigRevision::INITIAL
    );

    let (candidate, commit) = prepared.into_parts();
    commit.finish().await.expect("commit candidate");
    let committed = store
        .load_configuration()
        .await
        .expect("committed configuration");
    assert_eq!(committed.revision(), candidate.revision());
    assert_eq!(
        committed.settings().scheduler().on_rate_limited(),
        RateLimitMode::Reject
    );
}

#[tokio::test]
async fn explicit_rollback_discards_changed_candidate() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");

    let prepared = store
        .prepare_configuration(
            ConfigRevision::INITIAL,
            ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Set {
                    key: SettingKey::SchedulerFallbackOnRateLimit,
                    value: SettingValue::Boolean(true),
                }],
            },
        )
        .await
        .expect("prepared candidate");
    let (_, commit) = prepared.into_parts();
    commit.rollback().await.expect("rollback candidate");

    let current = store
        .load_configuration()
        .await
        .expect("current configuration");
    assert_eq!(current.revision(), ConfigRevision::INITIAL);
    assert_eq!(
        current
            .settings()
            .override_value(SettingKey::SchedulerFallbackOnRateLimit),
        None
    );
}

#[tokio::test]
async fn no_op_finish_rolls_back_without_advancing_revision() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("configuration.sqlite3"))
        .await
        .expect("store");

    let prepared = store
        .prepare_configuration(
            ConfigRevision::INITIAL,
            ConfigurationMutation::ApplySettingChanges {
                changes: vec![SettingOverrideChange::Reset {
                    key: SettingKey::SchedulerFallbackOnRateLimit,
                }],
            },
        )
        .await
        .expect("prepared no-op");
    assert!(!prepared.changed());
    assert_eq!(prepared.candidate().revision(), ConfigRevision::INITIAL);
    let (_, commit) = prepared.into_parts();
    commit.finish().await.expect("finish no-op");

    assert_eq!(
        store
            .load_configuration()
            .await
            .expect("current configuration")
            .revision(),
        ConfigRevision::INITIAL
    );
}
