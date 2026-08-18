import type { SettingItem } from "../api/settings-contracts";

const labels: Record<string, string> = {
  "admin.remote_enabled": "允许远程管理",
  "admin.session.idle_timeout": "会话空闲超时",
  "admin.session.absolute_timeout": "会话绝对超时",
  "admin.login.failure_window": "登录失败窗口",
  "admin.login.max_failures": "最大登录失败次数",
  "network.max_connections": "入站连接数上限",
  "network.request_header_timeout": "HTTP 请求头读取超时",
  "network.request_body_idle_timeout": "请求体空闲超时",
  "network.trusted_proxy_cidrs": "可信反向代理地址",
  "models.allowed": "客户端可使用模型",
  "affinity.enabled": "启用会话粘性",
  "affinity.ttl": "会话绑定 TTL",
  "affinity.wait_timeout": "会话绑定等待超时",
  "logs.request.enabled": "启用请求日志",
  "logs.request.retention": "请求日志保留时间",
  "logs.request.max_rows": "请求日志最大行数",
  "logs.http_access.max_rows": "系统日志最大行数",
  "logs.http_access.max_exchange_bytes": "系统日志原始交换容量",
  "logs.file.level": "文件日志级别",
  "logs.file.retention": "文件日志保留时间",
  "logs.file.max_total_size": "文件日志最大容量",
  "logs.telemetry_queue_capacity": "遥测队列容量",
  "logs.telemetry_queue_max_bytes": "遥测在途内存上限",
  "oauth.refresh.scan_interval": "OAuth 刷新扫描间隔",
  "oauth.refresh.lead_time": "OAuth 提前刷新窗口",
  "upstream.read_timeout": "上游读取超时",
  "upstream.strict_ssrf": "严格 SSRF 本地 DNS",
  "stream.precommit.max_bytes": "SSE 单帧与预提交字节上限",
  "stream.precommit.max_duration": "预提交最长等待",
  "stream.postcommit.idle_timeout": "提交后流空闲超时",
  "shutdown.request_grace_period": "请求排空宽限期",
  "shutdown.finalize_timeout": "最终收尾超时",
  "scheduler.on_rate_limited": "RPM 用尽行为",
  "scheduler.queue_timeout": "排队超时",
  "scheduler.max_waiting_requests": "最大排队数量",
  "scheduler.fallback_on_rate_limit": "RPM 用尽进入 fallback",
  "retry.precommit_total_budget": "提交前总预算",
};

const descriptions: Record<string, string> = {
  "logs.http_access.max_exchange_bytes":
    "原始请求/响应 Header 与 Body 捕获的总容量预算；超出后删除最旧的完整记录。",
  "logs.file.max_total_size":
    "本地 JSONL 日志目录允许占用的最大总容量。",
  "logs.telemetry_queue_max_bytes":
    "请求遥测在途内存上限；满载时直接丢弃并计数。",
  "stream.precommit.max_bytes":
    "每个 SSE 帧及首事件提交前允许的最大数据量。",
};

export function settingLabel(item: SettingItem) {
  return labels[item.key] ?? item.key;
}

export function settingDescription(item: SettingItem) {
  return descriptions[item.key] ?? item.description;
}

export function enumOptionLabel(value: string) {
  if (value === "wait") {
    return "等待";
  }
  if (value === "reject") {
    return "立即拒绝";
  }
  return value;
}

/** Placeholder inside numeric inputs — plain default value (seconds for durations). */
export function formatSettingDefaultPlaceholder(item: SettingItem) {
  return String(item.defaultValue);
}

export function reloadLabel(item: SettingItem) {
  if (item.applyMode === "restart_required") {
    return "修改后需要重启";
  }
  return null;
}
