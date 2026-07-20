use super::{SettingDefinition, SettingKey};

mod admin;
mod affinity;
mod logging;
mod reliability;
mod scheduler;

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::AdminRemoteEnabled
        | SettingKey::AdminSessionIdleTimeout
        | SettingKey::AdminSessionAbsoluteTimeout
        | SettingKey::AdminLoginFailureWindow
        | SettingKey::AdminLoginMaxFailures => admin::definition(key),
        SettingKey::AffinitySoftEnabled
        | SettingKey::AffinitySoftMode
        | SettingKey::AffinitySoftTtl
        | SettingKey::AffinityHardTtl
        | SettingKey::AffinitySoftPreferWaitTimeout
        | SettingKey::AffinityFixedWaitTimeout => affinity::definition(key),
        SettingKey::LogsRequestEnabled
        | SettingKey::LogsRequestRetention
        | SettingKey::LogsRequestMaxRows
        | SettingKey::LogsTelemetryQueueCapacity => logging::definition(key),
        SettingKey::SchedulerOnSaturated
        | SettingKey::SchedulerQueueTimeout
        | SettingKey::SchedulerMaxWaitingRequests
        | SettingKey::SchedulerFallbackOnSaturation
        | SettingKey::SchedulerAuxiliaryGlobalConcurrency
        | SettingKey::SchedulerAuxiliaryPerCredentialConcurrency => scheduler::definition(key),
        _ => reliability::definition(key),
    }
}
