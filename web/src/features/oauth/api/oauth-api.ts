import { requestJson } from "@/shared/api/http-client";

import {
  parseOAuthActivationResult,
  parseOAuthAccountConfiguration,
  parseOAuthDevicePollResult,
  parseOAuthImportResult,
  parseOAuthStartResult,
  type OAuthAccountModelsInput,
  type OAuthAccountUpdateInput,
  type OAuthProvider,
} from "./oauth-contracts";
import {
  parseNullableOAuthQuotaSnapshot,
  parseOAuthQuotaResetResult,
  parseOAuthQuotaSnapshot,
} from "./oauth-quota-contracts";

export function startOAuthLogin(provider: OAuthProvider) {
  return requestJson<unknown>("/api/admin/oauth/start", {
    method: "POST",
    body: { provider },
    timeoutMs: 35_000,
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

export function pollOAuthDevice(sessionId: string, signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/oauth/device/poll", {
    method: "POST",
    body: { session_id: sessionId },
    signal,
    timeoutMs: 35_000,
  }).then(parseOAuthDevicePollResult);
}

export function importOAuthFiles(files: readonly File[]) {
  const form = new FormData();
  for (const file of files) {
    form.append("files", file, file.name);
  }
  return requestJson<unknown>("/api/admin/oauth/import", {
    method: "POST",
    body: form,
    timeoutMs: 60_000,
  }).then(parseOAuthImportResult);
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
      requests_per_minute: input.requestsPerMinute,
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

export function getOAuthAccountQuota(id: string) {
  return requestJson<unknown>(
    `${accountCollection}/${encodeURIComponent(id)}/quota`,
  ).then(parseNullableOAuthQuotaSnapshot);
}

export function refreshOAuthAccountQuotaRequest(id: string) {
  return requestJson<unknown>(
    `${accountCollection}/${encodeURIComponent(id)}/quota/refresh`,
    { method: "POST" },
  ).then(parseOAuthQuotaSnapshot);
}

export function resetOAuthAccountQuota(id: string) {
  return requestJson<unknown>(
    `${accountCollection}/${encodeURIComponent(id)}/quota/reset`,
    { method: "POST" },
  ).then(parseOAuthQuotaResetResult);
}
