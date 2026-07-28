import { requestJson } from "@/shared/api/http-client";

import {
  type GatewayApiKeyCollectionResponse,
  type GatewayApiKeyConfiguration,
  type GatewayApiKeyCreateInput,
  type GatewayApiKeyCreateRequest,
  type GatewayApiKeyDeleteInput,
  type GatewayApiKeyDeleteRequest,
  type GatewayApiKeyRotateInput,
  type GatewayApiKeyRotateRequest,
  type GatewayApiKeyUpdateInput,
  type GatewayApiKeyUpdateRequest,
  parseGatewayApiKeyConfiguration,
} from "./gateway-api-key-contracts";

const collection = "/api/admin/gateway-api-keys";

export function listGatewayApiKeys(signal?: AbortSignal): Promise<GatewayApiKeyConfiguration> {
  return requestJson<GatewayApiKeyCollectionResponse>(collection, { signal }).then(
    parseGatewayApiKeyConfiguration,
  );
}

export function createGatewayApiKey(
  input: GatewayApiKeyCreateInput,
): Promise<GatewayApiKeyConfiguration> {
  const body: GatewayApiKeyCreateRequest = {
    expected_revision: input.expectedRevision,
    name: input.name,
    enabled: input.enabled,
  };
  return requestJson<GatewayApiKeyCollectionResponse>(collection, {
    method: "POST",
    body,
  }).then(parseGatewayApiKeyConfiguration);
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
): Promise<GatewayApiKeyConfiguration> {
  const body: GatewayApiKeyRotateRequest = {
    expected_revision: input.expectedRevision,
    expected_config_version: input.expectedConfigVersion,
    expected_token_version: input.expectedTokenVersion,
  };
  return requestJson<GatewayApiKeyCollectionResponse>(
    `${collection}/${encodeURIComponent(id)}/rotate`,
    {
      method: "POST",
      body,
    },
  ).then(parseGatewayApiKeyConfiguration);
}

export function deleteGatewayApiKey(
  id: string,
  input: GatewayApiKeyDeleteInput,
): Promise<GatewayApiKeyConfiguration> {
  const query: GatewayApiKeyDeleteRequest = {
    expected_revision: input.expectedRevision,
    expected_config_version: input.expectedConfigVersion,
  };
  const params = new URLSearchParams({
    expected_revision: String(query.expected_revision),
    expected_config_version: String(query.expected_config_version),
  });
  return requestJson<GatewayApiKeyCollectionResponse>(
    `${collection}/${encodeURIComponent(id)}?${params.toString()}`,
    {
      method: "DELETE",
    },
  ).then(parseGatewayApiKeyConfiguration);
}
