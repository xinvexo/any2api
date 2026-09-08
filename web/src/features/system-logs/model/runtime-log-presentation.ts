import type { RuntimeLogEntry } from "@/shared/api/generated/RuntimeLogEntry";
import type { RuntimeLogLevel } from "@/shared/api/generated/RuntimeLogLevel";
import { formatCompactDateTime } from "@/shared/lib/date-time";

export const levelLabels: Record<RuntimeLogLevel, string> = {
  error: "错误", warn: "警告", info: "信息", debug: "调试", trace: "跟踪",
};

export const moduleLabels: Record<string, string> = {
  oauth: "账号授权", configuration: "配置发布", storage: "数据存储", logging: "日志记录",
  client_version: "客户端版本", update: "应用更新", http: "HTTP 服务", application: "应用生命周期", runtime: "运行任务",
};

export const diagnosticLabels: Record<string, string> = {
  oauth_account_id: "账号 ID", oauth_account_name: "账号名称", provider: "供应商",
  request_id: "请求 ID", error: "错误原因", refresh_trigger: "触发方式",
  refresh_stage: "失败阶段", refresh_reason: "失败原因", upstream_status: "上游状态码",
  failure_scope: "影响范围", reauthorization_required: "需要重新授权", token_version: "凭据版本",
  occurred_at: "发生时间", records: "影响记录数", prepared_account_count: "影响账号数",
  config_revision: "配置版本", address: "监听地址", event: "事件类型",
};

export function eventTitle(entry: RuntimeLogEntry) {
  return entry.summary;
}

export function eventReason(entry: RuntimeLogEntry) {
  if (entry.fields.reauthorization_required === "true") return "需要重新授权，请到账号页面重新登录";
  return entry.fields.error ?? entry.fields.refresh_reason ?? "";
}

export function diagnosticValue(key: string, value: string) {
  if (key === "reauthorization_required") return value === "true" ? "是" : "否";
  if (key === "refresh_trigger") return ({ Scheduled: "定时刷新", AuthenticationFailure: "鉴权失败" } as Record<string, string>)[value] ?? value;
  if (key === "occurred_at" && Number.isFinite(Number(value))) return formatCompactDateTime(Number(value) * 1_000);
  return value;
}
