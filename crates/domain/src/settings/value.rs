use std::net::IpAddr;

use ipnet::IpNet;
use serde_json::{Value, json};
use thiserror::Error;

use super::{CodexQuotaRateCard, SettingKey, SettingValueType};
use crate::PublicModelName;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RateLimitMode {
    Wait,
    Reject,
}

impl RateLimitMode {
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
pub enum FileLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl FileLogLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "error" => Some(Self::Error),
            "warn" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelAccess {
    All,
    Allowlist(Vec<String>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingValue {
    Boolean(bool),
    Integer(u64),
    DurationSecs(u64),
    RateLimitMode(RateLimitMode),
    FileLogLevel(FileLogLevel),
    ModelAccess(ModelAccess),
    StringList(Vec<String>),
    CodexRateCard(CodexQuotaRateCard),
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
            SettingValueType::DurationSecs => value
                .as_u64()
                .map(Self::DurationSecs)
                .ok_or(SettingsValidationError::InvalidType),
            SettingValueType::Enum => {
                let value = value.as_str().ok_or(SettingsValidationError::InvalidType)?;
                if !definition.allowed_values().contains(&value) {
                    return Err(SettingsValidationError::InvalidEnum);
                }
                parse_enum(key, value).ok_or(SettingsValidationError::InvalidEnum)
            }
            SettingValueType::ModelAccess => parse_model_access(value).map(Self::ModelAccess),
            SettingValueType::StringList => parse_string_list(value).map(Self::StringList),
            SettingValueType::CodexRateCard => {
                CodexQuotaRateCard::from_json(value).map(Self::CodexRateCard)
            }
        }?;
        normalize_value(key, parsed)
    }

    #[must_use]
    pub fn to_json(self) -> Value {
        match self {
            Self::Boolean(value) => json!(value),
            Self::Integer(value) | Self::DurationSecs(value) => json!(value),
            Self::RateLimitMode(value) => json!(value.as_str()),
            Self::FileLogLevel(value) => json!(value.as_str()),
            Self::ModelAccess(ModelAccess::All) => json!("all"),
            Self::ModelAccess(ModelAccess::Allowlist(value)) => json!(value),
            Self::StringList(value) => json!(value),
            Self::CodexRateCard(value) => value.to_json(),
        }
    }

    pub const fn value_type(&self) -> SettingValueType {
        match self {
            Self::Boolean(_) => SettingValueType::Boolean,
            Self::Integer(_) => SettingValueType::Integer,
            Self::DurationSecs(_) => SettingValueType::DurationSecs,
            Self::RateLimitMode(_) | Self::FileLogLevel(_) => SettingValueType::Enum,
            Self::ModelAccess(_) => SettingValueType::ModelAccess,
            Self::StringList(_) => SettingValueType::StringList,
            Self::CodexRateCard(_) => SettingValueType::CodexRateCard,
        }
    }

    fn enum_value(&self) -> Option<&'static str> {
        match self {
            Self::RateLimitMode(value) => Some(value.as_str()),
            Self::FileLogLevel(value) => Some(value.as_str()),
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
    #[error("setting list contains an invalid value")]
    InvalidListValue,
    #[error("setting value is outside its allowed range")]
    OutOfRange,
    #[error("setting values form an invalid combination")]
    InvalidCombination,
}

pub(super) fn validate_value(
    key: SettingKey,
    value: &SettingValue,
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
        let min = numeric(&min).ok_or(SettingsValidationError::InvalidType)?;
        let max = numeric(&max).ok_or(SettingsValidationError::InvalidType)?;
        if !(min..=max).contains(&value) {
            return Err(SettingsValidationError::OutOfRange);
        }
    }
    Ok(())
}

pub(super) fn normalize_value(
    key: SettingKey,
    value: SettingValue,
) -> Result<SettingValue, SettingsValidationError> {
    let value = match (key, value) {
        (SettingKey::ModelsAllowed, SettingValue::ModelAccess(ModelAccess::Allowlist(values))) => {
            SettingValue::ModelAccess(ModelAccess::Allowlist(normalize_models(values)?))
        }
        (SettingKey::NetworkTrustedProxyCidrs, SettingValue::StringList(values)) => {
            SettingValue::StringList(normalize_trusted_proxy_cidrs(values)?)
        }
        (SettingKey::OAuthCodexRateCard, SettingValue::CodexRateCard(value)) => {
            value.validate()?;
            SettingValue::CodexRateCard(value)
        }
        (_, value) => value,
    };
    validate_value(key, &value)?;
    Ok(value)
}

pub(super) fn boolean(value: SettingValue) -> Result<bool, SettingsValidationError> {
    match value {
        SettingValue::Boolean(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

pub(super) fn integer(value: SettingValue) -> Result<u64, SettingsValidationError> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationSecs(value) => Ok(value),
        _ => Err(SettingsValidationError::InvalidType),
    }
}

fn parse_enum(key: SettingKey, value: &str) -> Option<SettingValue> {
    match key {
        SettingKey::SchedulerOnRateLimited => {
            RateLimitMode::parse(value).map(SettingValue::RateLimitMode)
        }
        SettingKey::LogsFileLevel => FileLogLevel::parse(value).map(SettingValue::FileLogLevel),
        _ => None,
    }
}

fn numeric(value: &SettingValue) -> Option<u64> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationSecs(value) => Some(*value),
        _ => None,
    }
}

fn parse_string_list(value: &Value) -> Result<Vec<String>, SettingsValidationError> {
    value
        .as_array()
        .ok_or(SettingsValidationError::InvalidType)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(SettingsValidationError::InvalidType)
        })
        .collect()
}

fn parse_model_access(value: &Value) -> Result<ModelAccess, SettingsValidationError> {
    if value.as_str() == Some("all") {
        return Ok(ModelAccess::All);
    }
    parse_string_list(value).map(ModelAccess::Allowlist)
}

fn normalize_models(values: Vec<String>) -> Result<Vec<String>, SettingsValidationError> {
    let mut values = values
        .into_iter()
        .map(|value| {
            PublicModelName::new(value)
                .map(|name| name.as_str().to_owned())
                .map_err(|_| SettingsValidationError::InvalidListValue)
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.sort();
    values.dedup();
    Ok(values)
}

fn normalize_trusted_proxy_cidrs(
    values: Vec<String>,
) -> Result<Vec<String>, SettingsValidationError> {
    let mut networks = values
        .into_iter()
        .map(|value| {
            let value = value.trim();
            value
                .parse::<IpNet>()
                .or_else(|_| value.parse::<IpAddr>().map(IpNet::from))
                .map(|network| network.trunc())
                .map_err(|_| SettingsValidationError::InvalidListValue)
        })
        .collect::<Result<Vec<_>, _>>()?;
    networks.sort();
    networks.dedup();
    Ok(networks
        .into_iter()
        .map(|network| network.to_string())
        .collect())
}
