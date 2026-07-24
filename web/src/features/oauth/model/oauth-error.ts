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
  oauth_account_busy: "OAuth 账号当前并发已满。",
  oauth_quota_unsupported: "该 OAuth Provider 不支持额度管理。",
  oauth_quota_reset_unavailable: "当前没有可用的额度重置次数。",
  oauth_quota_timeout: "额度查询超时。",
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
