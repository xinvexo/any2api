use crate::settings::{
    SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{
        MAX_SETTING_DURATION_SECS, definition as setting_definition, duration_definition,
    },
};

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::RetryMaxTotalAttempts => integer(
            key,
            3,
            1,
            10,
            "重试预算",
            "一个请求在下游提交前允许执行的最大上游 Attempt 数量。",
        ),
        SettingKey::RetryMaxCredentialSwitches => integer(
            key,
            2,
            0,
            9,
            "重试预算",
            "一个请求最多切换到其他 Credential 的次数。",
        ),
        SettingKey::RetryMaxSameCredentialRetries => integer(
            key,
            1,
            0,
            9,
            "重试预算",
            "同一个 Credential 在安全失败后允许额外重试的次数。",
        ),
        SettingKey::RetryPrecommitTotalBudget => duration(
            key,
            20,
            1,
            MAX_SETTING_DURATION_SECS,
            "重试预算",
            "从第一次 Attempt 开始到下游提交前可用于重试和退避的总时间。",
        ),
        SettingKey::RetryBaseDelay => duration(
            key,
            0,
            0,
            MAX_SETTING_DURATION_SECS,
            "重试退避",
            "第一次可重试失败后的基础等待时间。",
        ),
        SettingKey::RetryMaxDelay => duration(
            key,
            2,
            0,
            MAX_SETTING_DURATION_SECS,
            "重试退避",
            "指数退避的单次等待上限。",
        ),
        SettingKey::RetryJitterRatio => integer(
            key,
            20,
            0,
            100,
            "重试退避",
            "退避抖动百分比，使用 0 到 100 的整数。",
        ),
        SettingKey::CooldownRateLimitFallback => duration(
            key,
            60,
            1,
            MAX_SETTING_DURATION_SECS,
            "冷却",
            "上游 429 没有有效 Retry-After 时的模型级冷却时间。",
        ),
        SettingKey::CooldownModelUnsupported => duration(
            key,
            3_600,
            1,
            MAX_SETTING_DURATION_SECS,
            "冷却",
            "确认模型不可用后，该 Credential 与模型组合的冷却时间。",
        ),
        SettingKey::CooldownPermissionDenied => duration(
            key,
            900,
            1,
            MAX_SETTING_DURATION_SECS,
            "冷却",
            "权限或额度错误后当前 Credential generation 的冷却时间。",
        ),
        SettingKey::CooldownTransientEndpoint => duration(
            key,
            15,
            1,
            MAX_SETTING_DURATION_SECS,
            "冷却",
            "暂时性上游 Endpoint 错误的短冷却时间。",
        ),
        SettingKey::BreakerEndpointFailureThreshold => integer(
            key,
            3,
            1,
            100,
            "Endpoint 熔断",
            "失败窗口内打开 Endpoint 熔断器所需的连续失败次数。",
        ),
        SettingKey::BreakerEndpointFailureWindow => duration(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "Endpoint 熔断",
            "Endpoint 失败计数的滑动时间窗口。",
        ),
        SettingKey::BreakerEndpointOpenDuration => duration(
            key,
            15,
            1,
            MAX_SETTING_DURATION_SECS,
            "Endpoint 熔断",
            "Endpoint 熔断器打开后等待半开探测的时间。",
        ),
        SettingKey::BreakerProxyFailureThreshold => integer(
            key,
            3,
            1,
            100,
            "代理熔断",
            "失败窗口内打开代理熔断器所需的连接失败次数。",
        ),
        SettingKey::BreakerProxyFailureWindow => duration(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "代理熔断",
            "代理连接失败计数的滑动时间窗口。",
        ),
        SettingKey::BreakerProxyOpenDuration => duration(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "代理熔断",
            "代理熔断器打开后等待半开探测的时间。",
        ),
        SettingKey::BreakerHalfOpenMaxProbes => integer(
            key,
            1,
            1,
            100,
            "熔断探测",
            "单个 Endpoint 或代理处于 HalfOpen 时允许的并发探测数量。",
        ),
        _ => unreachable!(),
    }
}

const fn integer(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    setting_definition(
        key,
        SettingValueType::Integer,
        SettingValue::Integer(default),
        (
            Some(SettingValue::Integer(min)),
            Some(SettingValue::Integer(max)),
        ),
        &[],
        (group, description),
    )
}

const fn duration(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    duration_definition(key, default, min, max, group, description)
}
