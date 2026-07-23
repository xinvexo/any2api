import { ApiError } from "@/shared/api/http-client";

export function getProviderErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return messages[error.code] ?? "Provider 配置操作失败";
  }
  return error instanceof Error ? error.message : "Provider 配置操作失败";
}

const messages: Record<string, string> = {
  revision_conflict: "配置已发生变化，请刷新后重试。",
  provider_credential_version_conflict: "此 API Key 已被修改，请重新打开后再保存。",
  provider_credential_secret_version_conflict: "此 API Key 已被轮换，请刷新后重试。",
  provider_credential_label_conflict: "同一 Endpoint 下不能使用重复名称。",
  provider_credential_not_found: "API Key 不存在或已被删除。",
  invalid_provider_credential: "API Key 配置无效。",
  invalid_provider_api_key: "上游 API Key 格式无效。",
  provider_credential_disabled: "已停用的 API Key 不能测试。",
  provider_endpoint_disabled: "Endpoint 已停用，不能测试 API Key。",
  provider_credential_proxy_unavailable: "API Key 的实际代理不可用。",
  provider_credential_at_capacity: "API Key 当前并发已满，请稍后重试。",
  provider_credential_test_unavailable: "API Key 测试服务不可用。",
  provider_oauth_unavailable: "OAuth 登录服务不可用。",
  provider_oauth_unsupported: "这个 Provider 不支持 OAuth 登录。",
  provider_oauth_proxy_unavailable: "所选代理当前不可用。",
  provider_oauth_session_capacity: "当前 OAuth 登录会话过多，请稍后重试。",
  provider_oauth_session_invalid: "OAuth 登录会话无效或已经使用，请重新开始。",
  provider_oauth_session_expired: "OAuth 登录会话已过期，请重新开始。",
  provider_oauth_callback_invalid: "请粘贴浏览器地址栏中的完整 OAuth 回调 URL。",
  provider_oauth_authorization_denied: "上游账号拒绝了 OAuth 授权。",
  provider_oauth_state_mismatch: "这条回调不属于当前 OAuth 登录会话。",
  provider_oauth_configuration_changed: "登录期间 Provider 或代理发生变化，请重新开始。",
  provider_oauth_upstream_failed: "OAuth 上游没有完成 Token 交换，请检查网络或代理后重试。",
  proxy_referenced: "该代理仍被 API Key 使用，不能删除。",
  provider_endpoint_in_use: "该 Endpoint 仍有 API Key，不能删除。",
  provider_endpoint_identity_in_use: "已有 API Key 时不能修改 Provider 类型或协议。",
};
