use super::{SettingKey, SettingValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingOverrideChange {
    Set {
        key: SettingKey,
        value: SettingValue,
    },
    Reset {
        key: SettingKey,
    },
}

impl SettingOverrideChange {
    pub const fn key(&self) -> SettingKey {
        match self {
            Self::Set { key, .. } | Self::Reset { key } => *key,
        }
    }
}
