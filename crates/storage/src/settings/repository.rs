use any2api_domain::{
    ConfigRevision, SettingKey, SettingOverrides, SettingValue, SettingsConfiguration,
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

enum SettingMutation {
    Set {
        key: SettingKey,
        value: SettingValue,
    },
    Reset {
        key: SettingKey,
    },
}

#[async_trait]
impl SettingRepository for SqliteStore {
    async fn set_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
        value: SettingValue,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_setting(expected, SettingMutation::Set { key, value })
            .await
    }

    async fn reset_setting_override(
        &self,
        expected: ConfigRevision,
        key: SettingKey,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_setting(expected, SettingMutation::Reset { key })
            .await
    }
}

impl SqliteStore {
    async fn mutate_setting(
        &self,
        expected: ConfigRevision,
        mutation: SettingMutation,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_connection(&mut transaction, self.secret_vault(), expected, mutation).await?;
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
    mutation: SettingMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some((expected_settings, change)) = prepare_change(
        current.settings(),
        current.model_routes(),
        current.oauth_accounts(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    match change {
        PreparedSettingChange::Set { key, value } => {
            upsert_setting_override(connection, key, value).await?;
        }
        PreparedSettingChange::Reset { key } => {
            delete_setting_override(connection, key).await?;
        }
    }
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.settings(), &expected_settings);
    Ok((configuration, true))
}

enum PreparedSettingChange {
    Set {
        key: SettingKey,
        value: SettingValue,
    },
    Reset {
        key: SettingKey,
    },
}

fn prepare_change(
    current: &SettingsConfiguration,
    routes: &any2api_domain::ModelRouteConfiguration,
    accounts: &any2api_domain::OAuthAccountConfiguration,
    mutation: SettingMutation,
) -> Result<Option<(SettingsConfiguration, PreparedSettingChange)>, StorageError> {
    let mut overrides = SettingOverrides::from_entries(current.overrides().iter())?;
    let change = match mutation {
        SettingMutation::Set { key, value } => {
            let previous = overrides.get(key);
            overrides.insert(key, value)?;
            let normalized = restrict_model_allowlist(
                overrides
                    .get(key)
                    .expect("the setting override was just inserted"),
                routes,
                accounts,
            );
            overrides.insert(key, normalized.clone())?;
            if previous == Some(normalized.clone()) {
                return Ok(None);
            }
            PreparedSettingChange::Set {
                key,
                value: normalized,
            }
        }
        SettingMutation::Reset { key } => {
            if overrides.remove(key).is_none() {
                return Ok(None);
            }
            PreparedSettingChange::Reset { key }
        }
    };
    let settings = SettingsConfiguration::from_overrides(overrides)?;
    Ok(Some((settings, change)))
}
