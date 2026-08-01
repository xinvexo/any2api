use any2api_domain::{
    ConfigRevision, SettingKey, SettingOverrideChange, SettingOverrides, SettingsConfiguration,
};
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
};

use super::{
    model_allowlist::restrict_model_allowlist,
    rows::{delete_setting_override, upsert_setting_override},
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    changes: Vec<SettingOverrideChange>,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
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
    let configuration = load_configuration_from(connection).await?;
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
