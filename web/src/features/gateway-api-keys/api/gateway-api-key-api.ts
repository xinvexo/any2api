import { requestJson } from "@/shared/api/http-client";

import {
  type GatewayApiKeyConfiguration,
  type GatewayApiKeyCreateInput,
  type GatewayApiKeyRevokeInput,
  type GatewayApiKeyRotateInput,
  type GatewayApiKeySecretReceipt,
  type GatewayApiKeyUpdateInput,
  parseGatewayApiKeyConfiguration,
  parseGatewayApiKeySecretReceipt,
} from "./gateway-api-key-contracts";

const collection = "/api/admin/gateway-api-keys";

export function listGatewayApiKeys(signal?: AbortSignal): Promise<GatewayApiKeyConfiguration> {
  return requestJson<unknown>(collection, { signal }).then(parseGatewayApiKeyConfiguration);
}

export function createGatewayApiKey(
  input: GatewayApiKeyCreateInput,
): Promise<GatewayApiKeySecretReceipt> {
  return requestJson<unknown>(collection, {
    method: "POST",
    body: {
      expected_revision: input.expectedRevision,
      name: input.name,
      enabled: input.enabled,
    },
  }).then(parseGatewayApiKeySecretReceipt);
}

export function updateGatewayApiKey(
  id: string,
  input: GatewayApiKeyUpdateInput,
): Promise<GatewayApiKeyConfiguration> {
  return requestJson<unknown>(`${collection}/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      name: input.name,
      enabled: input.enabled,
    },
  }).then(parseGatewayApiKeyConfiguration);
}

export function rotateGatewayApiKey(
  id: string,
  input: GatewayApiKeyRotateInput,
): Promise<GatewayApiKeySecretReceipt> {
  return requestJson<unknown>(`${collection}/${encodeURIComponent(id)}/rotate`, {
    method: "POST",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      expected_token_version: input.expectedTokenVersion,
    },
  }).then(parseGatewayApiKeySecretReceipt);
}

export function revokeGatewayApiKey(
  id: string,
  input: GatewayApiKeyRevokeInput,
): Promise<GatewayApiKeyConfiguration> {
  return requestJson<unknown>(`${collection}/${encodeURIComponent(id)}/revoke`, {
    method: "POST",
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
    },
  }).then(parseGatewayApiKeyConfiguration);
}
