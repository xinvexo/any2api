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
  proxy_referenced: "该代理仍被 API Key 使用，不能删除。",
  provider_endpoint_in_use: "该 Endpoint 仍有 API Key，不能删除。",
  provider_endpoint_identity_in_use: "已有 API Key 时不能修改 Provider 类型或协议。",
};
