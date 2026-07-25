use any2api_domain::{SettingKey, SettingOverrides, SettingValue, SettingsConfiguration};
use sqlx::{FromRow, SqliteConnection};

use crate::error::StorageError;

#[derive(Debug, FromRow)]
struct SettingOverrideRow {
    key: String,
    value_json: String,
}

pub(crate) async fn load_settings_from(
    connection: &mut SqliteConnection,
) -> Result<SettingsConfiguration, StorageError> {
    let rows = sqlx::query_as::<_, SettingOverrideRow>(
        "SELECT key, value_json FROM setting_overrides ORDER BY key ASC",
    )
    .fetch_all(connection)
    .await?;
    let entries = rows
        .into_iter()
        .map(parse_row)
        .collect::<Result<Vec<_>, _>>()?;
    let overrides =
        SettingOverrides::from_entries(entries).map_err(|_| StorageError::CorruptConfiguration)?;
    SettingsConfiguration::from_overrides(overrides).map_err(|_| StorageError::CorruptConfiguration)
}

pub(crate) async fn upsert_setting_override(
    connection: &mut SqliteConnection,
    key: SettingKey,
    value: SettingValue,
) -> Result<(), StorageError> {
    let value_json =
        serde_json::to_string(&value.to_json()).map_err(|_| StorageError::CorruptConfiguration)?;
    sqlx::query(
        "INSERT INTO setting_overrides (key, value_json) VALUES (?, ?) \
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, \
         updated_at = CURRENT_TIMESTAMP",
    )
    .bind(key.as_str())
    .bind(value_json)
    .execute(connection)
    .await?;
    Ok(())
}

pub(crate) async fn delete_setting_override(
    connection: &mut SqliteConnection,
    key: SettingKey,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM setting_overrides WHERE key = ?")
        .bind(key.as_str())
        .execute(connection)
        .await?;
    Ok(())
}

fn parse_row(row: SettingOverrideRow) -> Result<(SettingKey, SettingValue), StorageError> {
    let key = SettingKey::parse(&row.key).ok_or(StorageError::CorruptConfiguration)?;
    let json =
        serde_json::from_str(&row.value_json).map_err(|_| StorageError::CorruptConfiguration)?;
    let value =
        SettingValue::from_json(key, &json).map_err(|_| StorageError::CorruptConfiguration)?;
    Ok((key, value))
}
