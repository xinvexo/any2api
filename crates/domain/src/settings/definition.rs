use super::{SettingKey, SettingValue};

pub const MAX_SETTING_DURATION_SECS: u64 = 86_400;
pub const MAX_SETTING_COUNT: u64 = 100_000;
pub const MAX_SETTING_AUXILIARY: u64 = 10_000;
pub const MAX_AFFINITY_TTL_SECS: u64 = 2_592_000;

pub(super) const fn definition(
    key: SettingKey,
    value_type: SettingValueType,
    default: SettingValue,
    bounds: (Option<SettingValue>, Option<SettingValue>),
    allowed_values: &'static [&'static str],
    presentation: (&'static str, &'static str),
) -> SettingDefinition {
    SettingDefinition {
        key,
        value_type,
        default,
        min: bounds.0,
        max: bounds.1,
        allowed_values,
        apply_mode: SettingApplyMode::HotReload,
        web_group: presentation.0,
        description: presentation.1,
    }
}

pub(super) const fn duration_definition(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    web_group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    definition(
        key,
        SettingValueType::DurationSecs,
        SettingValue::DurationSecs(default),
        (
            Some(SettingValue::DurationSecs(min)),
            Some(SettingValue::DurationSecs(max)),
        ),
        &[],
        (web_group, description),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettingValueType {
    Boolean,
    Integer,
    DurationSecs,
    Enum,
}

impl SettingValueType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::DurationSecs => "duration_secs",
            Self::Enum => "enum",
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SettingDefinition {
    pub(super) key: SettingKey,
    pub(super) value_type: SettingValueType,
    pub(super) default: SettingValue,
    pub(super) min: Option<SettingValue>,
    pub(super) max: Option<SettingValue>,
    pub(super) allowed_values: &'static [&'static str],
    pub(super) apply_mode: SettingApplyMode,
    pub(super) web_group: &'static str,
    pub(super) description: &'static str,
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
