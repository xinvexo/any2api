use any2api_domain::{ConfigRevision, SettingDefinition, SettingKey, SettingValue};
use any2api_runtime::api::PublishedSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    config_revision: u64,
    items: Vec<SettingResponse>,
}

impl SettingsResponse {
    pub(crate) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        let items = SettingKey::ALL
            .into_iter()
            .map(|key| SettingResponse::from_definition(key.definition(), snapshot))
            .collect();
        Self {
            config_revision: snapshot.revision().get(),
            items,
        }
    }
}

#[derive(Debug, Serialize)]
struct SettingResponse {
    key: &'static str,
    value_type: &'static str,
    default_value: Value,
    override_value: Option<Value>,
    effective_value: Value,
    min_value: Option<Value>,
    max_value: Option<Value>,
    allowed_values: Option<Vec<&'static str>>,
    apply_mode: &'static str,
    web_group: &'static str,
    description: &'static str,
}

impl SettingResponse {
    fn from_definition(definition: SettingDefinition, snapshot: &PublishedSnapshot) -> Self {
        let key = definition.key();
        Self {
            key: key.as_str(),
            value_type: definition.value_type().as_str(),
            default_value: definition.default().to_json(),
            override_value: snapshot
                .settings()
                .override_value(key)
                .map(SettingValue::to_json),
            effective_value: snapshot.settings().effective_value(key).to_json(),
            min_value: definition.min().map(SettingValue::to_json),
            max_value: definition.max().map(SettingValue::to_json),
            allowed_values: (!definition.allowed_values().is_empty())
                .then(|| definition.allowed_values().to_vec()),
            apply_mode: definition.apply_mode().as_str(),
            web_group: definition.web_group(),
            description: definition.description(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingWriteRequest {
    expected_revision: u64,
    value: Value,
}

impl SettingWriteRequest {
    pub(crate) fn into_domain(
        self,
        key: SettingKey,
    ) -> Result<(ConfigRevision, SettingValue), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let value = SettingValue::from_json(key, &self.value)
            .map_err(|error| AdminApiError::invalid_setting(error.to_string()))?;
        Ok((revision, value))
    }
}
