use serde_json::{Value, json};
use thiserror::Error;

pub const MAX_SETTING_DURATION_MS: u64 = 86_400_000;
pub const MAX_SETTING_COUNT: u64 = 100_000;
pub const MAX_SETTING_AUXILIARY: u64 = 10_000;
const SATURATION_ALLOWED_VALUES: &[&str] = &["wait", "reject"];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SettingKey {
    SchedulerOnSaturated,
    SchedulerQueueTimeout,
    SchedulerMaxWaitingRequests,
    SchedulerFallbackOnSaturation,
    SchedulerAuxiliaryGlobalConcurrency,
    SchedulerAuxiliaryPerCredentialConcurrency,
}

impl SettingKey {
    pub const ALL: [Self; 6] = [
        Self::SchedulerOnSaturated,
        Self::SchedulerQueueTimeout,
        Self::SchedulerMaxWaitingRequests,
        Self::SchedulerFallbackOnSaturation,
        Self::SchedulerAuxiliaryGlobalConcurrency,
        Self::SchedulerAuxiliaryPerCredentialConcurrency,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchedulerOnSaturated => "scheduler.on_saturated",
            Self::SchedulerQueueTimeout => "scheduler.queue_timeout",
            Self::SchedulerMaxWaitingRequests => "scheduler.max_waiting_requests",
            Self::SchedulerFallbackOnSaturation => "scheduler.fallback_on_saturation",
            Self::SchedulerAuxiliaryGlobalConcurrency => "scheduler.auxiliary_global_concurrency",
            Self::SchedulerAuxiliaryPerCredentialConcurrency => {
                "scheduler.auxiliary_per_credential_concurrency"
            }
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.as_str() == value)
    }

    pub const fn definition(self) -> SettingDefinition {
        match self {
            Self::SchedulerOnSaturated => SettingDefinition {
                key: self,
                value_type: SettingValueType::Enum,
                default: SettingValue::Saturation(SaturationMode::Wait),
                min: None,
                max: None,
                allowed_values: SATURATION_ALLOWED_VALUES,
                apply_mode: SettingApplyMode::HotReload,
                web_group: "排队策略",
                description: "所有可用 Credential 都达到并发上限时，等待容量或立即拒绝请求。",
            },
            Self::SchedulerQueueTimeout => SettingDefinition {
                key: self,
                value_type: SettingValueType::DurationMs,
                default: SettingValue::DurationMs(30_000),
                min: Some(SettingValue::DurationMs(1)),
                max: Some(SettingValue::DurationMs(MAX_SETTING_DURATION_MS)),
                allowed_values: &[],
                apply_mode: SettingApplyMode::HotReload,
                web_group: "排队策略",
                description: "生成请求等待可用并发槽位的最长时间。",
            },
            Self::SchedulerMaxWaitingRequests => SettingDefinition {
                key: self,
                value_type: SettingValueType::Integer,
                default: SettingValue::Integer(128),
                min: Some(SettingValue::Integer(1)),
                max: Some(SettingValue::Integer(MAX_SETTING_COUNT)),
                allowed_values: &[],
                apply_mode: SettingApplyMode::HotReload,
                web_group: "排队策略",
                description: "允许同时等待可用并发槽位的生成请求数量。",
            },
            Self::SchedulerFallbackOnSaturation => SettingDefinition {
                key: self,
                value_type: SettingValueType::Boolean,
                default: SettingValue::Boolean(false),
                min: None,
                max: None,
                allowed_values: &[],
                apply_mode: SettingApplyMode::HotReload,
                web_group: "排队策略",
                description: "主 tier 满载时，是否允许继续选择 fallback tier。",
            },
            Self::SchedulerAuxiliaryGlobalConcurrency => SettingDefinition {
                key: self,
                value_type: SettingValueType::Integer,
                default: SettingValue::Integer(32),
                min: Some(SettingValue::Integer(1)),
                max: Some(SettingValue::Integer(MAX_SETTING_AUXILIARY)),
                allowed_values: &[],
                apply_mode: SettingApplyMode::HotReload,
                web_group: "辅助请求",
                description: "count_tokens 等辅助请求在整个进程内的并发上限。",
            },
            Self::SchedulerAuxiliaryPerCredentialConcurrency => SettingDefinition {
                key: self,
                value_type: SettingValueType::Integer,
                default: SettingValue::Integer(4),
                min: Some(SettingValue::Integer(1)),
                max: Some(SettingValue::Integer(MAX_SETTING_AUXILIARY)),
                allowed_values: &[],
                apply_mode: SettingApplyMode::HotReload,
                web_group: "辅助请求",
                description: "单个 Credential 执行辅助请求的并发上限。",
            },
        }
    }
}

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
pub enum SettingValueType {
    Boolean,
    Integer,
    DurationMs,
    Enum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingApplyMode {
    HotReload,
    RestartRequired,
}

impl SettingApplyMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HotReload => "hot_reload",
            Self::RestartRequired => "restart_required",
        }
    }
}

impl SettingValueType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::DurationMs => "duration_ms",
            Self::Enum => "enum",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingValue {
    Boolean(bool),
    Integer(u64),
    DurationMs(u64),
    Saturation(SaturationMode),
}

impl SettingValue {
    pub fn from_json(key: SettingKey, value: &Value) -> Result<Self, SettingsValidationError> {
        let parsed = match key.definition().value_type() {
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
                if !key.definition().allowed_values().contains(&value) {
                    return Err(SettingsValidationError::InvalidEnum);
                }
                SaturationMode::parse(value)
                    .map(Self::Saturation)
                    .ok_or(SettingsValidationError::InvalidEnum)
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
        }
    }

    pub const fn value_type(self) -> SettingValueType {
        match self {
            Self::Boolean(_) => SettingValueType::Boolean,
            Self::Integer(_) => SettingValueType::Integer,
            Self::DurationMs(_) => SettingValueType::DurationMs,
            Self::Saturation(_) => SettingValueType::Enum,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingDefinition {
    key: SettingKey,
    value_type: SettingValueType,
    default: SettingValue,
    min: Option<SettingValue>,
    max: Option<SettingValue>,
    allowed_values: &'static [&'static str],
    apply_mode: SettingApplyMode,
    web_group: &'static str,
    description: &'static str,
}

impl SettingDefinition {
    pub const fn key(self) -> SettingKey {
        self.key
    }

    pub const fn value_type(self) -> SettingValueType {
        self.value_type
    }

    pub const fn default(self) -> SettingValue {
        self.default
    }

    pub const fn min(self) -> Option<SettingValue> {
        self.min
    }

    pub const fn max(self) -> Option<SettingValue> {
        self.max
    }

    pub const fn allowed_values(self) -> &'static [&'static str] {
        self.allowed_values
    }

    pub const fn apply_mode(self) -> SettingApplyMode {
        self.apply_mode
    }

    pub const fn web_group(self) -> &'static str {
        self.web_group
    }

    pub const fn description(self) -> &'static str {
        self.description
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
    if value.value_type() != key.definition().value_type() {
        return Err(SettingsValidationError::InvalidType);
    }
    if let (Some(min), Some(max)) = (key.definition().min(), key.definition().max()) {
        let value = numeric(value).ok_or(SettingsValidationError::InvalidType)?;
        let min = numeric(min).ok_or(SettingsValidationError::InvalidType)?;
        let max = numeric(max).ok_or(SettingsValidationError::InvalidType)?;
        if !(min..=max).contains(&value) {
            return Err(SettingsValidationError::OutOfRange);
        }
    }
    Ok(())
}

fn numeric(value: SettingValue) -> Option<u64> {
    match value {
        SettingValue::Integer(value) | SettingValue::DurationMs(value) => Some(value),
        SettingValue::Boolean(_) | SettingValue::Saturation(_) => None,
    }
}
