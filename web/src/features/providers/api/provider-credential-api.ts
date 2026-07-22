import { requestJson } from "@/shared/api/http-client";

import {
  type ProviderCredentialConfiguration,
  type ProviderCredentialCreateInput,
  type ProviderCredentialRotateInput,
  type ProviderCredentialTestResult,
  type ProviderCredentialModelsInput,
  type ProviderCredentialUpdateInput,
  parseProviderCredentialConfiguration,
  parseProviderCredentialTestResult,
} from "./provider-credential-contracts";

const endpointCollection = "/api/admin/provider-endpoints";
const credentialCollection = "/api/admin/provider-credentials";

export function listProviderCredentials(endpointId: string, signal?: AbortSignal) {
  return requestJson<unknown>(
    `${endpointCollection}/${encodeURIComponent(endpointId)}/credentials`,
    { signal },
  ).then(parseProviderCredentialConfiguration);
}

export function createProviderCredential(
  endpointId: string,
  input: ProviderCredentialCreateInput,
) {
  return requestJson<unknown>(
    `${endpointCollection}/${encodeURIComponent(endpointId)}/credentials`,
    {
      method: "POST",
      body: {
        expected_revision: input.expectedRevision,
        label: input.label,
        credential_kind: "api_key",
        api_key: input.apiKey,
        proxy_profile_id: input.proxyProfileId,
        max_concurrency: input.maxConcurrency,
        enabled: input.enabled,
      },
    },
  ).then(parseProviderCredentialConfiguration);
}

export function updateProviderCredential(id: string, input: ProviderCredentialUpdateInput) {
  return requestJson<unknown>(`${credentialCollection}/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      label: input.label,
      proxy_profile_id: input.proxyProfileId,
      max_concurrency: input.maxConcurrency,
      enabled: input.enabled,
    },
  }).then(parseProviderCredentialConfiguration);
}

export function rotateProviderCredential(id: string, input: ProviderCredentialRotateInput) {
  return requestJson<unknown>(
    `${credentialCollection}/${encodeURIComponent(id)}/rotate-secret`,
    {
      method: "POST",
      body: {
        expected_revision: input.expectedRevision,
        expected_config_version: input.expectedConfigVersion,
        expected_secret_version: input.expectedSecretVersion,
        api_key: input.apiKey,
      },
    },
  ).then(parseProviderCredentialConfiguration);
}

export function testProviderCredential(id: string): Promise<ProviderCredentialTestResult> {
  return requestJson<unknown>(
    `${credentialCollection}/${encodeURIComponent(id)}/test`,
    { method: "POST", timeoutMs: 30_000 },
  ).then(parseProviderCredentialTestResult);
}

export function setProviderCredentialModels(
  id: string,
  input: ProviderCredentialModelsInput,
): Promise<ProviderCredentialConfiguration> {
  return requestJson<unknown>(`${credentialCollection}/${encodeURIComponent(id)}/models`, {
    method: "PUT",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      models: input.models,
    },
  }).then(parseProviderCredentialConfiguration);
}

export function deleteProviderCredential(
  id: string,
  expectedRevision: number,
  expectedConfigVersion: number,
): Promise<ProviderCredentialConfiguration> {
  const query = new URLSearchParams({
    expected_revision: String(expectedRevision),
    expected_config_version: String(expectedConfigVersion),
  });
  return requestJson<unknown>(
    `${credentialCollection}/${encodeURIComponent(id)}?${query.toString()}`,
    { method: "DELETE" },
  ).then(parseProviderCredentialConfiguration);
}
