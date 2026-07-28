use std::collections::BTreeSet;

use any2api_domain::{
    ModelRouteConfiguration, OAuthAccountConfiguration, SettingKey, SettingValue,
    SettingsConfiguration,
};
use sqlx::SqliteConnection;

use crate::error::StorageError;

use super::rows::upsert_setting_override;

pub(super) fn restrict_model_allowlist(
    value: SettingValue,
    routes: &ModelRouteConfiguration,
    accounts: &OAuthAccountConfiguration,
) -> SettingValue {
    let SettingValue::OptionalStringList(Some(mut values)) = value else {
        return value;
    };
    let published = configured_public_models(routes, accounts);
    values.retain(|value| published.contains(value));
    SettingValue::OptionalStringList(Some(values))
}

pub(crate) async fn prune_model_allowlist(
    connection: &mut SqliteConnection,
    settings: &SettingsConfiguration,
    routes: &ModelRouteConfiguration,
    accounts: &OAuthAccountConfiguration,
) -> Result<bool, StorageError> {
    let Some(current @ SettingValue::OptionalStringList(_)) =
        settings.override_value(SettingKey::ModelsAllowed)
    else {
        return Ok(false);
    };
    let pruned = restrict_model_allowlist(current.clone(), routes, accounts);
    if pruned == current {
        return Ok(false);
    }
    upsert_setting_override(connection, SettingKey::ModelsAllowed, pruned).await?;
    Ok(true)
}

fn configured_public_models(
    routes: &ModelRouteConfiguration,
    accounts: &OAuthAccountConfiguration,
) -> BTreeSet<String> {
    let mut models = routes
        .routes()
        .iter()
        .filter(|route| route.enabled())
        .map(|route| route.public_model().as_str().to_owned())
        .collect::<BTreeSet<_>>();
    for account in accounts.accounts() {
        models.extend(
            account
                .models()
                .iter()
                .map(|model| model.as_str().to_owned()),
        );
    }
    models
}
