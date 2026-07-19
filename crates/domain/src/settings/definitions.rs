use super::{SettingDefinition, SettingKey};

mod affinity;
mod reliability;
mod scheduler;

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AffinitySoftEnabled
        | SettingKey::AffinitySoftMode
        | SettingKey::AffinitySoftTtl
        | SettingKey::AffinityHardTtl
        | SettingKey::AffinitySoftPreferWaitTimeout
        | SettingKey::AffinityFixedWaitTimeout => affinity::definition(key),
        SettingKey::SchedulerOnSaturated
        | SettingKey::SchedulerQueueTimeout
        | SettingKey::SchedulerMaxWaitingRequests
        | SettingKey::SchedulerFallbackOnSaturation
        | SettingKey::SchedulerAuxiliaryGlobalConcurrency
        | SettingKey::SchedulerAuxiliaryPerCredentialConcurrency => scheduler::definition(key),
        _ => reliability::definition(key),
    }
}
