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
  oauth_unavailable: "OAuth2 login is unavailable.",
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
