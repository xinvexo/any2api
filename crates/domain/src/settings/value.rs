use serde_json::{Value, json};
use thiserror::Error;

use super::{SettingKey, SettingValueType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaturationMode {
    Wait,
    Reject,
}

impl SaturationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wait => "wait",
            Self::Reject => "reject",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "wait" => Some(Self::Wait),
            "reject" => Some(Self::Reject),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffinityMode {
    Prefer,
    Strict,
}

impl AffinityMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prefer => "prefer",
            Self::Strict => "strict",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "prefer" => Some(Self::Prefer),
            "strict" => Some(Self::Strict),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingValue {
    Boolean(bool),
    Integer(u64),
    DurationMs(u64),
    Saturation(SaturationMode),
    AffinityMode(AffinityMode),
}

impl SettingValue {
    pub fn from_json(key: SettingKey, value: &Value) -> Result<Self, SettingsValidationError> {
        let definition = key.definition();
        let parsed = match definition.value_type() {
            SettingValueType::Boolean => value
                .as_bool()
                .map(Self::Boolean)
                .ok_or(SettingsValidationError::InvalidType),
            SettingValueType::Integer => value
                .as_u64()
                .map(Self::Integer)
                .ok_or(SettingsValidationError::InvalidType),
            SettingValueType::DurationMs => value
                .as_u64()
                .map(Self::DurationMs)
                .ok_or(SettingsValidationError::InvalidType),
            SettingValueType::Enum => {
                let value = value.as_str().ok_or(SettingsValidationError::InvalidType)?;
                if !definition.allowed_values().contains(&value) {
                    return Err(SettingsValidationError::InvalidEnum);
                }
                parse_enum(key, value).ok_or(SettingsValidationError::InvalidEnum)
            }
        }?;
        validate_value(key, parsed)?;
        Ok(parsed)
    }

    #[must_use]
    pub fn to_json(self) -> Value {
        match self {
            Self::Boolean(value) => json!(value),
            Self::Integer(value) | Self::DurationMs(value) => json!(value),
            Self::Saturation(value) => json!(value.as_str()),
            Self::AffinityMode(value) => json!(value.as_str()),
        }
    }

    pub const fn value_type(self) -> SettingValueType {
        match self {
            Self::Boolean(_) => SettingValueType::Boolean,
            Self::Integer(_) => SettingValueType::Integer,
            Self::DurationMs(_) => SettingValueType::DurationMs,
            Self::Saturation(_) | Self::AffinityMode(_) => SettingValueType::Enum,
        }
    }

    fn enum_value(self) -> Option<&'static str> {
        match self {
            Self::Saturation(value) => Some(value.as_str()),
            Self::AffinityMode(value) => Some(value.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum SettingsValidationError {
    #[error("setting value has the wrong type")]
    InvalidType,
    #[error("setting enum value is invalid")]
    InvalidEnum,
    #[error("setting value is outside its allowed range")]
    OutOfRange,
}

pub(super) fn validate_value(
    key: SettingKey,
    value: SettingValue,
) -> Result<(), SettingsValidationError> {
    let definition = key.definition();
    if value.value_type() != definition.value_type() {
        return Err(SettingsValidationError::InvalidType);
    }
    if definition.value_type() == SettingValueType::Enum
        && !value
            .enum_value()
            .is_some_and(|value| definition.allowed_values().contains(&value))
    {
        return Err(SettingsValidationError::InvalidEnum);
    }
    if let (Some(min), Some(max)) = (definition.min(), definition.max()) {
        let value = numeric(value).ok_or(SettingsValidationError::InvalidType)?;
        let min = numeric(min).ok_or(SettingsValidationError::InvalidType)?;
        let max = numeric(max).ok_or(SettingsValidationError::InvalidType)?;
        if !(min..=max).contains(&value) {
            return Err(SettingsValidationError::OutOfRange);
        }
    }
    Ok(())
}

pub(super) fn boolean(value: SettingValue) -> Result<bool, SettingsValidationError> {
    match value {
        SettingValue::Boolean(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

pub(super) fn integer(value: SettingValue) -> Result<u64, SettingsValidationError> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationMs(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

fn parse_enum(key: SettingKey, value: &str) -> Option<SettingValue> {
    match key {
        SettingKey::SchedulerOnSaturated => {
            SaturationMode::parse(value).map(SettingValue::Saturation)
        }
        SettingKey::AffinitySoftMode => AffinityMode::parse(value).map(SettingValue::AffinityMode),
        _ => None,
    }
}

fn numeric(value: SettingValue) -> Option<u64> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationMs(value) => Some(value),
        _ => None,
    }
}
