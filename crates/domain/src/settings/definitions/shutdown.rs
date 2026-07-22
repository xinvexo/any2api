use crate::settings::{SettingDefinition, SettingKey, definition::duration_definition};

pub(super) const fn definition(key: SettingKey) -> SettingDefinition {
    match key {
        SettingKey::ShutdownRequestGracePeriod => duration_definition(
            key,
            30,
            1,
            300,
            "优雅停机",
            "停止接收新请求后，等待活动 HTTP 请求自然完成的最长时间。",
        ),
        SettingKey::ShutdownFinalizeTimeout => duration_definition(
            key,
            5,
            1,
            60,
            "优雅停机",
            "强制取消、后台任务、遥测与 SQLite 最终收尾的单阶段最长时间。",
        ),
        _ => unreachable!(),
    }
}
