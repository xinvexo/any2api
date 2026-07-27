import { ApiError } from "@/shared/api/http-client";

const messages: Record<string, string> = {
  oauth_session_capacity: "Too many OAuth2 login sessions are active.",
  oauth_session_invalid: "This OAuth2 login session is invalid or was already used.",
  oauth_session_expired: "This OAuth2 login session expired. Start again.",
  oauth_callback_invalid: "The callback URL is invalid.",
  oauth_authorization_denied: "OAuth2 authorization was denied.",
  oauth_state_mismatch: "The callback URL does not match this login session.",
  oauth_token_timeout: "The token endpoint timed out.",
  oauth_token_exchange_failed: "The token exchange failed.",
  oauth_activation_failed: "The login completed, but the account could not be activated.",
  oauth_unavailable: "OAuth2 login is unavailable.",
  oauth_import_no_files: "请选择至少一个 OAuth JSON 文件。",
  oauth_import_too_many_files: "一次最多导入 32 个 JSON 文件。",
  oauth_import_file_too_large: "单个 JSON 文件不能超过 2 MiB。",
  oauth_import_total_too_large: "所选 JSON 文件总大小不能超过 8 MiB。",
  oauth_import_too_many_accounts: "一次最多导入 2,000 个 OAuth 账号。",
  oauth_import_invalid_json: "上传内容不是有效的 JSON。",
  oauth_import_invalid_envelope: "JSON 的账号包装格式不受支持。",
  oauth_import_empty: "JSON 中没有 OAuth 账号。",
  oauth_import_unsupported_account: "JSON 中包含不受支持或非 OAuth 的账号。",
  oauth_import_ambiguous_account: "JSON 中的账号无法唯一识别 Provider。",
  oauth_import_invalid_account: "JSON 中的 OAuth 认证信息无效。",
  oauth_import_activation_failed: "认证信息有效，但账号未能写入并激活。",
  oauth_account_not_found: "OAuth 账号不存在或已被删除。",
  oauth_account_version_conflict: "OAuth 账号已被其他操作更新，请确认最新内容后重试。",
  oauth_model_unavailable: "所选模型不在该 OAuth 账号的可用目录中。",
  oauth_account_rate_limited: "OAuth 账号的本地 RPM 已用尽。",
  oauth_quota_unsupported: "该 OAuth Provider 不支持额度管理。",
  oauth_quota_reset_unavailable: "当前没有可用的额度重置次数。",
  oauth_quota_timeout: "额度查询超时。",
  oauth_account_authentication_unverified: "账号认证无法确认：上游返回 401，但 Token 刷新未完成。",
  oauth_account_authentication_failed: "账号认证已失效：刷新 Token 后仍被上游拒绝。",
  oauth_account_restricted: "账号访问被上游限制或封禁。",
  oauth_quota_upstream_failed: "上游额度请求失败。",
  oauth_quota_unavailable: "OAuth 额度管理当前不可用。",
};

export function getOAuthErrorMessage(error: unknown) {
  if (error instanceof ApiError) {
    return messages[error.code] ?? error.message;
  }
  if (error instanceof Error && error.message === "request timed out") {
    return "The OAuth2 request timed out.";
  }
  return "OAuth2 login failed.";
}
