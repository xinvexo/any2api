import { requestJson } from "@/shared/api/http-client";

import {
  parseOAuthActivationResult,
  parseOAuthAccountConfiguration,
  parseOAuthStartResult,
  type OAuthAccountModelsInput,
  type OAuthAccountUpdateInput,
  type OAuthProvider,
} from "./oauth-contracts";

export function startOAuthLogin(provider: OAuthProvider) {
  return requestJson<unknown>("/api/admin/oauth/start", {
    method: "POST",
    body: { provider },
  }).then(parseOAuthStartResult);
}

export function exchangeOAuthCallback(sessionId: string, callbackUrl: string) {
  return requestJson<unknown>("/api/admin/oauth/exchange", {
    method: "POST",
    body: {
      session_id: sessionId,
      callback_url: callbackUrl,
    },
  }).then(parseOAuthActivationResult);
}

const accountCollection = "/api/admin/oauth/accounts";

export function listOAuthAccounts(signal?: AbortSignal) {
  return requestJson<unknown>(accountCollection, { signal }).then(
    parseOAuthAccountConfiguration,
  );
}

export function updateOAuthAccount(id: string, input: OAuthAccountUpdateInput) {
  return requestJson<unknown>(`${accountCollection}/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      label: input.label,
      max_concurrency: input.maxConcurrency,
      enabled: input.enabled,
    },
  }).then(parseOAuthAccountConfiguration);
}

export function setOAuthAccountModels(id: string, input: OAuthAccountModelsInput) {
  return requestJson<unknown>(
    `${accountCollection}/${encodeURIComponent(id)}/models`,
    {
      method: "PUT",
      body: {
        expected_revision: input.expectedRevision,
        expected_config_version: input.expectedConfigVersion,
        models: input.models,
      },
    },
  ).then(parseOAuthAccountConfiguration);
}

export function deleteOAuthAccount(
  id: string,
  expectedRevision: number,
  expectedConfigVersion: number,
) {
  const query = new URLSearchParams({
    expected_revision: String(expectedRevision),
    expected_config_version: String(expectedConfigVersion),
  });
  return requestJson<unknown>(
    `${accountCollection}/${encodeURIComponent(id)}?${query.toString()}`,
    { method: "DELETE" },
  ).then(parseOAuthAccountConfiguration);
}
