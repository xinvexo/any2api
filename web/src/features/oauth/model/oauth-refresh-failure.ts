import type { ApiErrorDiagnostic } from "@/shared/api/http-client";

import type { OAuthRefreshFailure } from "../api/oauth-contracts";

export type OAuthRefreshFailureLike = OAuthRefreshFailure | ApiErrorDiagnostic;

const triggerLabels: Record<string, string> = {
  scheduled: "到期前定时刷新",
  authentication_failure: "Access Token 401 后刷新",
};

const stageLabels: Record<string, string> = {
  preflight: "刷新前检查",
  request_build: "构造刷新请求",
  dns: "DNS 解析",
  tcp: "TCP 连接",
  proxy_handshake: "代理握手",
  tls: "TLS 握手",
  write_request: "发送刷新请求",
  await_headers: "等待响应头",
  read_response: "读取响应体",
  token_endpoint: "Token Endpoint",
  parse_response: "解析 Token 响应",
  validate_token: "校验新 Token",
  publish_token: "发布新 Token",
  verify_authentication: "刷新后认证复核",
};

const reasonLabels: Record<string, string> = {
  account_unavailable: "账号在刷新期间已不可用",
  provider_unavailable: "Provider Driver 不可用",
  token_material_unavailable: "当前 Token 材料不可用",
  proxy_unavailable: "当前账号的 OAuth 出口不可用",
  refresh_token_missing: "账号没有 Refresh Token",
  request_invalid: "刷新请求无法构造",
  transport_failure: "网络传输失败",
  read_timeout: "读取刷新响应超时",
  response_too_large: "刷新响应超过大小限制",
  invalid_grant: "Refresh Token 已失效（invalid_grant）",
  refresh_token_expired: "Refresh Token 已过期",
  refresh_token_reused: "Refresh Token 已被重复使用",
  refresh_token_invalidated: "Refresh Token 已被撤销",
  upstream_rejected: "上游拒绝刷新，但未给出可确认的失效原因",
  invalid_response: "Token 响应格式无效",
  provider_mismatch: "新 Token 的 Provider 不匹配",
  routing_profile_invalid: "新 Token 的路由资料无效",
  document_serialization_failed: "新 Token 无法生成认证文档",
  publication_conflict: "Token 发布时配置已发生变化",
  publication_failed: "新 Token 发布失败",
  refresh_unavailable: "刷新操作未能取得可用账号状态",
  refreshed_access_token_rejected: "新 Access Token 仍被上游 401 拒绝",
};

const scopeLabels: Record<string, string> = {
  endpoint: "上游端点",
  proxy: "代理",
  egress_path: "上游端点 × 出口路径",
  unattributed: "网络路径未归因",
};

export function oauthRefreshTriggerLabel(value: string) {
  return triggerLabels[value] ?? value;
}

export function oauthRefreshStageLabel(value: string) {
  return stageLabels[value] ?? value;
}

export function oauthRefreshReasonLabel(value: string) {
  return reasonLabels[value] ?? value;
}

export function oauthRefreshScopeLabel(value: string | null) {
  return value === null ? null : (scopeLabels[value] ?? value);
}

export function formatOAuthRefreshFailure(failure: OAuthRefreshFailureLike) {
  const metadata = [
    failure.upstreamStatus === null ? null : `HTTP ${failure.upstreamStatus}`,
    oauthRefreshScopeLabel(failure.failureScope),
  ].filter((value): value is string => value !== null);
  const action = failure.reauthorizationRequired
    ? "需要重新授权此账号。"
    : "无需立即重新授权，请按阶段检查网络、代理或上游状态。";
  return [
    `触发：${oauthRefreshTriggerLabel(failure.trigger)}`,
    `阶段：${oauthRefreshStageLabel(failure.stage)}`,
    `错误：${oauthRefreshReasonLabel(failure.reason)}${metadata.length > 0 ? `（${metadata.join(" · ")}）` : ""}`,
    action,
  ].join("；");
}

export function formatOAuthRefreshFailureTime(occurredAt: number) {
  return new Date(occurredAt * 1_000).toLocaleString(undefined, {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
