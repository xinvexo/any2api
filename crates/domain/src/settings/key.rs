use super::{
    AffinityMode, SaturationMode, SettingApplyMode, SettingDefinition, SettingValue,
    SettingValueType,
    definition::{
        MAX_AFFINITY_TTL_MS, MAX_SETTING_AUXILIARY, MAX_SETTING_COUNT, MAX_SETTING_DURATION_MS,
    },
};

const SATURATION_ALLOWED_VALUES: &[&str] = &["wait", "reject"];
const AFFINITY_MODE_ALLOWED_VALUES: &[&str] = &["prefer", "strict"];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SettingKey {
    AffinitySoftEnabled,
    AffinitySoftMode,
    AffinitySoftTtl,
    AffinityHardTtl,
    AffinitySoftPreferWaitTimeout,
    AffinityFixedWaitTimeout,
    SchedulerOnSaturated,
    SchedulerQueueTimeout,
    SchedulerMaxWaitingRequests,
    SchedulerFallbackOnSaturation,
    SchedulerAuxiliaryGlobalConcurrency,
    SchedulerAuxiliaryPerCredentialConcurrency,
}

impl SettingKey {
    pub const ALL: [Self; 12] = [
        Self::AffinitySoftEnabled,
        Self::AffinitySoftMode,
        Self::AffinitySoftTtl,
        Self::AffinityHardTtl,
        Self::AffinitySoftPreferWaitTimeout,
        Self::AffinityFixedWaitTimeout,
        Self::SchedulerOnSaturated,
        Self::SchedulerQueueTimeout,
        Self::SchedulerMaxWaitingRequests,
        Self::SchedulerFallbackOnSaturation,
        Self::SchedulerAuxiliaryGlobalConcurrency,
        Self::SchedulerAuxiliaryPerCredentialConcurrency,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AffinitySoftEnabled => "affinity.soft.enabled",
            Self::AffinitySoftMode => "affinity.soft.mode",
            Self::AffinitySoftTtl => "affinity.soft.ttl",
            Self::AffinityHardTtl => "affinity.hard.ttl",
            Self::AffinitySoftPreferWaitTimeout => "affinity.soft.prefer_wait_timeout",
            Self::AffinityFixedWaitTimeout => "affinity.fixed_wait_timeout",
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
            Self::AffinitySoftEnabled => definition(
                self,
                SettingValueType::Boolean,
                SettingValue::Boolean(true),
                (None, None),
                &[],
                (
                    "软会话粘性",
                    "是否为没有 previous_response_id 的生成请求启用软会话粘性。",
                ),
            ),
            Self::AffinitySoftMode => definition(
                self,
                SettingValueType::Enum,
                SettingValue::AffinityMode(AffinityMode::Prefer),
                (None, None),
                AFFINITY_MODE_ALLOWED_VALUES,
                (
                    "软会话粘性",
                    "prefer 在等待超时后允许重绑，strict 只允许原 Credential。",
                ),
            ),
            Self::AffinitySoftTtl => duration_definition(
                self,
                3_600_000,
                1_000,
                MAX_AFFINITY_TTL_MS,
                "软会话粘性",
                "软会话绑定在当前进程内保持活跃的时间。",
            ),
            Self::AffinityHardTtl => duration_definition(
                self,
                86_400_000,
                1_000,
                MAX_AFFINITY_TTL_MS,
                "硬会话粘性",
                "Codex Response ID 与原 Credential 绑定的内存有效期。",
            ),
            Self::AffinitySoftPreferWaitTimeout => duration_definition(
                self,
                2_000,
                1,
                MAX_SETTING_DURATION_MS,
                "软会话粘性",
                "prefer 会话等待绑定 Credential 容量的最长时间。",
            ),
            Self::AffinityFixedWaitTimeout => duration_definition(
                self,
                30_000,
                1,
                MAX_SETTING_DURATION_MS,
                "固定会话等待",
                "硬绑定和 strict 会话等待原 Credential 容量的最长时间。",
            ),
            Self::SchedulerOnSaturated => definition(
                self,
                SettingValueType::Enum,
                SettingValue::Saturation(SaturationMode::Wait),
                (None, None),
                SATURATION_ALLOWED_VALUES,
                (
                    "排队策略",
                    "所有可用 Credential 都达到并发上限时，等待容量或立即拒绝请求。",
                ),
            ),
            Self::SchedulerQueueTimeout => duration_definition(
                self,
                30_000,
                1,
                MAX_SETTING_DURATION_MS,
                "排队策略",
                "生成请求等待可用并发槽位的最长时间。",
            ),
            Self::SchedulerMaxWaitingRequests => definition(
                self,
                SettingValueType::Integer,
                SettingValue::Integer(128),
                (
                    Some(SettingValue::Integer(1)),
                    Some(SettingValue::Integer(MAX_SETTING_COUNT)),
                ),
                &[],
                ("排队策略", "允许同时等待可用并发槽位的生成请求数量。"),
            ),
            Self::SchedulerFallbackOnSaturation => definition(
                self,
                SettingValueType::Boolean,
                SettingValue::Boolean(false),
                (None, None),
                &[],
                (
                    "排队策略",
                    "主 tier 满载时，是否允许继续选择 fallback tier。",
                ),
            ),
            Self::SchedulerAuxiliaryGlobalConcurrency => definition(
                self,
                SettingValueType::Integer,
                SettingValue::Integer(32),
                (
                    Some(SettingValue::Integer(1)),
                    Some(SettingValue::Integer(MAX_SETTING_AUXILIARY)),
                ),
                &[],
                (
                    "辅助请求",
                    "count_tokens 等辅助请求在整个进程内的并发上限。",
                ),
            ),
            Self::SchedulerAuxiliaryPerCredentialConcurrency => definition(
                self,
                SettingValueType::Integer,
                SettingValue::Integer(4),
                (
                    Some(SettingValue::Integer(1)),
                    Some(SettingValue::Integer(MAX_SETTING_AUXILIARY)),
                ),
                &[],
                ("辅助请求", "单个 Credential 执行辅助请求的并发上限。"),
            ),
        }
    }
}

const fn duration_definition(
    key: SettingKey,
    default: u64,
    min: u64,
    max: u64,
    web_group: &'static str,
    description: &'static str,
) -> SettingDefinition {
    definition(
        key,
        SettingValueType::DurationMs,
        SettingValue::DurationMs(default),
        (
            Some(SettingValue::DurationMs(min)),
            Some(SettingValue::DurationMs(max)),
        ),
        &[],
        (web_group, description),
    )
}

const fn definition(
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
