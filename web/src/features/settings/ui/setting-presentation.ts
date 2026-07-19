import type { SettingItem, SettingValue, SettingValueType } from "../api/settings-contracts";

const labels: Record<string, string> = {
  "scheduler.on_saturated": "满载行为",
  "scheduler.queue_timeout": "排队超时",
  "scheduler.max_waiting_requests": "最大排队数量",
  "scheduler.fallback_on_saturation": "满载进入 fallback",
  "scheduler.auxiliary_global_concurrency": "辅助请求全局并发",
  "scheduler.auxiliary_per_credential_concurrency": "辅助请求单 Credential 并发",
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
