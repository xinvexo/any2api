use crate::settings::{
    CodexQuotaRateCard, SettingDefinition, SettingKey, SettingValue, SettingValueType,
    definition::{MAX_SETTING_DURATION_SECS, duration_definition},
};

pub(super) fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::OAuthRefreshScanInterval => duration_definition(
            key,
            30,
            1,
            MAX_SETTING_DURATION_SECS,
            "OAuth 刷新",
            "扫描已启用 OAuth 账号是否进入提前刷新窗口的间隔。",
        ),
        SettingKey::OAuthRefreshLeadTime => duration_definition(
            key,
            300,
            1,
            MAX_SETTING_DURATION_SECS,
            "OAuth 刷新",
            "在访问 Token 到期前提前触发刷新的时间窗口。",
        ),
        SettingKey::OAuthCodexRateCard => crate::settings::definition::definition(
            key,
            SettingValueType::CodexRateCard,
            SettingValue::CodexRateCard(CodexQuotaRateCard::default()),
            (None, None),
            &[],
            (
                "额度费率",
                "Codex 本地额度遥测使用的费率卡；模型费率单位为每百万 Token 的 nano-Credits。",
            ),
        ),
        _ => unreachable!(),
    }
}
