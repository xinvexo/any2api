use any2api_domain::{
    ConfigRevision, SettingKey, SettingOverrideChange, SettingOverrides, SettingValue,
    SettingsConfiguration,
};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
    sqlite::SqliteStore,
};

use super::{
    model_allowlist::restrict_model_allowlist,
    rows::{delete_setting_override, upsert_setting_override},
};

#[async_trait]
pub trait SettingRepository: Send + Sync {
    async fn apply_setting_changes(
        &self,
        expected: ConfigRevision,
        changes: Vec<SettingOverrideChange>,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn reset_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl SettingRepository for SqliteStore {
    async fn apply_setting_changes(
        &self,
        expected: ConfigRevision,
        changes: Vec<SettingOverrideChange>,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_settings(expected, changes).await
    }

    async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_settings(expected, vec![SettingOverrideChange::Set { key, value }])
            .await
    }

    async fn reset_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_settings(expected, vec![SettingOverrideChange::Reset { key }])
            .await
    }
}

impl SqliteStore {
    async fn mutate_settings(
        &self,
        expected: ConfigRevision,
        changes: Vec<SettingOverrideChange>,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_connection(&mut transaction, self.secret_vault(), expected, changes).await?;
        if changed {
            transaction.commit().await?;
        } else {
            transaction.rollback().await?;
        }
        Ok(configuration)
    }
}

async fn mutate_connection(
    connection: &mut SqliteConnection,
    vault: &crate::vault::SecretVault,
    expected: ConfigRevision,
    changes: Vec<SettingOverrideChange>,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(expected_settings) = prepare_changes(
        current.settings(),
        current.model_routes(),
        current.oauth_accounts(),
        changes,
    )?
    else {
        return Ok((current, false));
    };
    persist_changes(connection, current.settings(), &expected_settings).await?;
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.settings(), &expected_settings);
    Ok((configuration, true))
}

fn prepare_changes(
    current: &SettingsConfiguration,
    routes: &any2api_domain::ModelRouteConfiguration,
    accounts: &any2api_domain::OAuthAccountConfiguration,
    changes: Vec<SettingOverrideChange>,
) -> Result<Option<SettingsConfiguration>, StorageError> {
    let mut overrides = SettingOverrides::from_entries(current.overrides().iter())?;
    for change in changes {
        match change {
            SettingOverrideChange::Set { key, value } => {
                let value = if key == SettingKey::ModelsAllowed {
                    restrict_model_allowlist(value, routes, accounts)
                } else {
                    value
                };
                overrides.insert(key, value)?;
            }
            SettingOverrideChange::Reset { key } => {
                overrides.remove(key);
            }
        }
    }
    let settings = SettingsConfiguration::from_overrides(overrides)?;
    Ok((settings != *current).then_some(settings))
}

async fn persist_changes(
    connection: &mut SqliteConnection,
    current: &SettingsConfiguration,
    next: &SettingsConfiguration,
) -> Result<(), StorageError> {
    for key in SettingKey::ALL {
        let previous = current.override_value(key);
        let value = next.override_value(key);
        if previous == value {
            continue;
        }
        if let Some(value) = value {
            upsert_setting_override(connection, key, value).await?;
        } else {
            delete_setting_override(connection, key).await?;
        }
    }
    Ok(())
}
