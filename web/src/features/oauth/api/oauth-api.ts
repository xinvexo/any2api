import { requestDownload, requestJson } from "@/shared/api/http-client";

import {
  parseOAuthStartResult,
  type OAuthProvider,
} from "./oauth-contracts";

export function startOAuthLogin(provider: OAuthProvider) {
  return requestJson<unknown>("/api/admin/oauth/start", {
    method: "POST",
    body: { provider },
  }).then(parseOAuthStartResult);
}

export function exchangeOAuthCallback(sessionId: string, callbackUrl: string) {
  return requestDownload("/api/admin/oauth/exchange", {
    method: "POST",
    body: {
      session_id: sessionId,
      callback_url: callbackUrl,
    },
  });
}
