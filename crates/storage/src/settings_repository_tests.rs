use any2api_domain::{ConfigRevision, SaturationMode, SettingKey, SettingValue};
use tempfile::tempdir;

use crate::{
    configuration_repository::ConfigurationRepository, error::StorageError,
    settings_repository::SettingRepository, sqlite::SqliteStore,
};

#[tokio::test]
async fn scheduler_overrides_persist_and_reset_to_compiled_defaults() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("settings.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let initial = store.load_configuration().await.expect("initial settings");

    assert_eq!(
        initial.settings().scheduler().on_saturated(),
        SaturationMode::Wait
    );
    assert_eq!(
        initial
            .settings()
            .override_value(SettingKey::SchedulerOnSaturated),
        None
    );

    let updated = store
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::SchedulerOnSaturated,
            SettingValue::Saturation(SaturationMode::Reject),
        )
        .await
        .expect("override setting");
    assert_eq!(updated.revision().get(), 2);
    assert_eq!(
        updated.settings().scheduler().on_saturated(),
        SaturationMode::Reject
    );

    let no_op = store
        .set_setting_override(
            updated.revision(),
            SettingKey::SchedulerOnSaturated,
            SettingValue::Saturation(SaturationMode::Reject),
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
        persisted.settings().scheduler().on_saturated(),
        SaturationMode::Reject
    );

    let reset = reopened
        .reset_setting_override(persisted.revision(), SettingKey::SchedulerOnSaturated)
        .await
        .expect("reset setting");
    assert_eq!(reset.revision().get(), 3);
    assert_eq!(
        reset.settings().scheduler().on_saturated(),
        SaturationMode::Wait
    );
    assert_eq!(
        reset
            .settings()
            .override_value(SettingKey::SchedulerOnSaturated),
        None
    );
}

#[tokio::test]
async fn explicit_override_equal_to_default_is_preserved() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("settings.sqlite3"))
        .await
        .expect("store");

    let updated = store
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::SchedulerFallbackOnSaturation,
            SettingValue::Boolean(false),
        )
        .await
        .expect("explicit default override");

    assert_eq!(updated.revision().get(), 2);
    assert_eq!(
        updated
            .settings()
            .override_value(SettingKey::SchedulerFallbackOnSaturation),
        Some(SettingValue::Boolean(false))
    );
}

#[tokio::test]
async fn stale_revision_and_corrupt_rows_fail_closed() {
    let directory = tempdir().expect("temporary directory");
    let store = SqliteStore::connect(&directory.path().join("settings.sqlite3"))
        .await
        .expect("store");
    let updated = store
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::SchedulerMaxWaitingRequests,
            SettingValue::Integer(64),
        )
        .await
        .expect("first override");

    let conflict = store
        .set_setting_override(
            ConfigRevision::INITIAL,
            SettingKey::SchedulerMaxWaitingRequests,
            SettingValue::Integer(32),
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
