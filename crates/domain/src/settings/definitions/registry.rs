use super::super::{SettingDefinition, SettingKey};
use super::{admin, affinity, logging, oauth, reliability, scheduler, shutdown, stream, upstream};

pub(in crate::settings) const fn definition(key: SettingKey) -> SettingDefinition {
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
        | SettingKey::LogsFileLevel
        | SettingKey::LogsFileRetention
        | SettingKey::LogsFileMaxTotalSize
        | SettingKey::LogsTelemetryQueueCapacity => logging::definition(key),
        SettingKey::OAuthRefreshScanInterval | SettingKey::OAuthRefreshLeadTime => {
            oauth::definition(key)
        }
        SettingKey::UpstreamReadTimeout | SettingKey::UpstreamStrictSsrf => {
            upstream::definition(key)
        }
        SettingKey::StreamPrecommitMaxBytes
        | SettingKey::StreamPrecommitMaxDuration
        | SettingKey::StreamPostcommitIdleTimeout => stream::definition(key),
        SettingKey::ShutdownRequestGracePeriod | SettingKey::ShutdownFinalizeTimeout => {
            shutdown::definition(key)
        }
        SettingKey::SchedulerOnRateLimited
        | SettingKey::SchedulerQueueTimeout
        | SettingKey::SchedulerMaxWaitingRequests
        | SettingKey::SchedulerFallbackOnRateLimit => scheduler::definition(key),
        _ => reliability::definition(key),
    }
}
