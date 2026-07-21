import type { SettingItem, SettingValue, SettingValueType } from "../api/settings-contracts";

const labels: Record<string, string> = {
  "admin.remote_enabled": "允许远程管理",
  "admin.session.idle_timeout": "会话空闲超时",
  "admin.session.absolute_timeout": "会话绝对超时",
  "admin.login.failure_window": "登录失败窗口",
  "admin.login.max_failures": "最大登录失败次数",
  "affinity.soft.enabled": "启用软粘性",
  "affinity.soft.mode": "软粘性模式",
  "affinity.soft.ttl": "软绑定 TTL",
  "affinity.hard.ttl": "硬绑定 TTL",
  "affinity.soft.prefer_wait_timeout": "Prefer 等待超时",
  "affinity.fixed_wait_timeout": "固定绑定等待超时",
  "logs.request.enabled": "启用请求日志",
  "logs.request.retention": "请求日志保留时间",
  "logs.request.max_rows": "请求日志最大行数",
  "logs.telemetry_queue_capacity": "遥测队列容量",
  "upstream.read_timeout": "上游读取超时",
  "upstream.strict_ssrf": "严格 SSRF 本地 DNS",
  "stream.precommit.max_bytes": "SSE 单帧与预提交字节上限",
  "stream.precommit.max_duration": "预提交最长等待",
  "stream.postcommit.idle_timeout": "提交后流空闲超时",
  "scheduler.on_saturated": "满载行为",
  "scheduler.queue_timeout": "排队超时",
  "scheduler.max_waiting_requests": "最大排队数量",
  "scheduler.fallback_on_saturation": "满载进入 fallback",
  "scheduler.auxiliary_global_concurrency": "辅助请求全局并发",
  "scheduler.auxiliary_per_credential_concurrency": "辅助请求单 Credential 并发",
  "retry.max_total_attempts": "最大总尝试次数",
  "retry.max_credential_switches": "最大 Credential 切换次数",
  "retry.max_same_credential_retries": "单 Credential 重试次数",
  "retry.precommit_total_budget": "提交前总预算",
  "retry.base_delay": "基础退避",
  "retry.max_delay": "最大退避",
  "retry.jitter_ratio": "退避抖动",
  "cooldown.rate_limit_fallback": "限流默认冷却",
  "cooldown.model_unsupported": "模型不可用冷却",
  "cooldown.permission_denied": "权限与额度冷却",
  "cooldown.transient_endpoint": "Endpoint 短冷却",
  "breaker.endpoint.failure_threshold": "Endpoint 失败阈值",
  "breaker.endpoint.failure_window": "Endpoint 失败窗口",
  "breaker.endpoint.open_duration": "Endpoint 打开时长",
  "breaker.proxy.failure_threshold": "代理失败阈值",
  "breaker.proxy.failure_window": "代理失败窗口",
  "breaker.proxy.open_duration": "代理打开时长",
  "breaker.half_open_max_probes": "半开探测并发",
};

export function settingLabel(item: SettingItem) {
  return labels[item.key] ?? item.key;
}

export function enumOptionLabel(value: string) {
  if (value === "wait") {
    return "等待";
  }
  if (value === "reject") {
    return "立即拒绝";
  }
  if (value === "prefer") {
    return "优先原 Credential，超时后重绑";
  }
  if (value === "strict") {
    return "只允许原 Credential";
  }
  return value;
}

export function formatSettingValue(value: SettingValue | null, type: SettingValueType) {
  if (value === null) {
    return "未覆盖";
  }
  if (type === "boolean") {
    return value ? "启用" : "关闭";
  }
  if (type === "enum") {
    return enumOptionLabel(String(value));
  }
  return type === "duration_ms" ? `${value} ms` : String(value);
}

export function reloadLabel(item: SettingItem) {
  if (item.applyMode === "restart_required") {
    return "修改后需要重启";
  }
  return "保存后立即生效";
}
