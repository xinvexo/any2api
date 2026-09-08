use std::{collections::BTreeMap, io};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum RuntimeLogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLogQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RuntimeLogEntry {
    pub id: String,
    pub timestamp: String,
    pub level: RuntimeLogLevel,
    pub module: String,
    pub target: String,
    pub message: String,
    pub summary: String,
    pub fields: BTreeMap<String, String>,
}

#[must_use]
pub fn runtime_log_summary(message: &str) -> &str {
    match message {
        "OAuth account token refresh failed" => "账号凭据刷新失败",
        "OAuth account token refreshed" => "账号凭据已刷新",
        "OAuth account refreshed after authentication failure" => "鉴权失败后已刷新账号凭据",
        "OAuth account refresh batch publication failed" => "账号凭据刷新后保存失败",
        "automatic OAuth quota refresh failed" => "自动同步账号额度失败",
        "manual OAuth model catalog refresh failed" => "刷新账号模型列表失败",
        "OAuth quota upstream request failed" => "查询上游额度失败",
        "OAuth quota request could not be constructed" => "额度查询请求创建失败",
        "OAuth model catalog initialization failed" => "账号模型列表初始化失败",
        "OAuth2 account activation failed" => "账号激活失败",
        "OAuth2 login could not be completed" => "账号登录未完成",
        "critical configuration publish task failed; terminating process" => {
            "配置发布任务异常，应用退出"
        }
        "request telemetry batch was dropped" => "请求日志写入失败，部分记录已丢失",
        "HTTP access log batch was dropped" => "HTTP 访问日志写入失败，部分记录已丢失",
        "HTTP access log retention cleanup failed" => "HTTP 访问日志清理失败",
        "HTTP access log storage reclaim failed" => "HTTP 访问日志空间回收失败",
        "request telemetry retention cleanup failed" => "请求日志清理失败",
        "request telemetry writer task failed" => "日志写入任务异常",
        "official client version persistence failed" => "客户端版本信息保存失败",
        "official client version fetch failed" => "客户端版本同步失败",
        "any2api is listening" => "应用已启动，开始接收请求",
        "any2api shutdown complete" => "应用已正常停止",
        "any2api shutdown incomplete; terminating process" => "应用停止未完成，进程退出",
        "administrator password initialized from environment" => "管理员密码已初始化",
        _ => message,
    }
}

#[derive(Debug, Default, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RuntimeLogPage {
    pub items: Vec<RuntimeLogEntry>,
    pub next_cursor: Option<String>,
}

/// Reads the application's retained diagnostic events without tying HTTP to a log writer.
#[async_trait]
pub trait RuntimeLogSource: Send + Sync {
    async fn list(&self, query: RuntimeLogQuery) -> io::Result<RuntimeLogPage>;
}

#[cfg(test)]
pub(crate) fn export_bindings(config: &ts_rs::Config) -> Result<(), ts_rs::ExportError> {
    <RuntimeLogPage as ts_rs::TS>::export_all(config)
}
