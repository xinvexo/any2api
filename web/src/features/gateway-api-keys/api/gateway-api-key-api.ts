import { requestJson } from "@/shared/api/http-client";

import {
  type GatewayApiKeyCollectionResponse,
  type GatewayApiKeyConfiguration,
  type GatewayApiKeyCreateInput,
  type GatewayApiKeyCreateRequest,
  type GatewayApiKeyRevokeInput,
  type GatewayApiKeyRevokeRequest,
  type GatewayApiKeyRotateInput,
  type GatewayApiKeyRotateRequest,
  type GatewayApiKeySecretReceipt,
  type GatewayApiKeySecretResponse,
  type GatewayApiKeyUpdateInput,
  type GatewayApiKeyUpdateRequest,
  parseGatewayApiKeyConfiguration,
  parseGatewayApiKeySecretReceipt,
} from "./gateway-api-key-contracts";

const collection = "/api/admin/gateway-api-keys";

export function listGatewayApiKeys(signal?: AbortSignal): Promise<GatewayApiKeyConfiguration> {
  return requestJson<GatewayApiKeyCollectionResponse>(collection, { signal }).then(
    parseGatewayApiKeyConfiguration,
  );
}

export function createGatewayApiKey(
  input: GatewayApiKeyCreateInput,
): Promise<GatewayApiKeySecretReceipt> {
  const body: GatewayApiKeyCreateRequest = {
    expected_revision: input.expectedRevision,
    name: input.name,
    enabled: input.enabled,
    token: input.token,
  };
  return requestJson<GatewayApiKeySecretResponse>(collection, {
    method: "POST",
    body,
  }).then(parseGatewayApiKeySecretReceipt);
}

export function updateGatewayApiKey(
  id: string,
  input: GatewayApiKeyUpdateInput,
): Promise<GatewayApiKeyConfiguration> {
  const body: GatewayApiKeyUpdateRequest = {
    expected_revision: input.expectedRevision,
    expected_config_version: input.expectedConfigVersion,
    name: input.name,
    enabled: input.enabled,
  };
  return requestJson<GatewayApiKeyCollectionResponse>(`${collection}/${encodeURIComponent(id)}`, {
    method: "PATCH",
    body,
  }).then(parseGatewayApiKeyConfiguration);
}

export function rotateGatewayApiKey(
  id: string,
  input: GatewayApiKeyRotateInput,
): Promise<GatewayApiKeySecretReceipt> {
  const body: GatewayApiKeyRotateRequest = {
    expected_revision: input.expectedRevision,
    expected_config_version: input.expectedConfigVersion,
    expected_token_version: input.expectedTokenVersion,
    token: input.token,
  };
  return requestJson<GatewayApiKeySecretResponse>(
    `${collection}/${encodeURIComponent(id)}/rotate`,
    {
      method: "POST",
      body,
    },
  ).then(parseGatewayApiKeySecretReceipt);
}

export function revokeGatewayApiKey(
  id: string,
  input: GatewayApiKeyRevokeInput,
): Promise<GatewayApiKeyConfiguration> {
  const body: GatewayApiKeyRevokeRequest = {
    expected_revision: input.expectedRevision,
    expected_config_version: input.expectedConfigVersion,
  };
  return requestJson<GatewayApiKeyCollectionResponse>(
    `${collection}/${encodeURIComponent(id)}/revoke`,
    {
      method: "POST",
      body,
    },
  ).then(parseGatewayApiKeyConfiguration);
}
