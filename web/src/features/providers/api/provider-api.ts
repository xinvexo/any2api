import { requestJson } from "@/shared/api/http-client";

import {
  type ProviderEndpointConfiguration,
  type ProviderEndpointWriteInput,
  parseProviderEndpointConfiguration,
} from "./provider-contracts";

const collectionPath = "/api/admin/provider-endpoints";

export function listProviderEndpoints(signal?: AbortSignal) {
  return requestJson<unknown>(collectionPath, { signal }).then(
    parseProviderEndpointConfiguration,
  );
}

export function createProviderEndpoint(input: ProviderEndpointWriteInput) {
  return writeProviderEndpoint(collectionPath, "POST", input);
}

export function updateProviderEndpoint(id: string, input: ProviderEndpointWriteInput) {
  return writeProviderEndpoint(
    `${collectionPath}/${encodeURIComponent(id)}`,
    "PATCH",
    input,
  );
}

export function deleteProviderEndpoint(id: string, expectedRevision: number) {
  return requestJson<unknown>(
    `${collectionPath}/${encodeURIComponent(id)}?expected_revision=${expectedRevision}`,
    { method: "DELETE" },
  ).then(parseProviderEndpointConfiguration);
}

function writeProviderEndpoint(
  path: string,
  method: string,
  input: ProviderEndpointWriteInput,
): Promise<ProviderEndpointConfiguration> {
  return requestJson<unknown>(path, {
    method,
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      name: input.name,
      provider_kind: input.providerKind,
      base_url: input.baseUrl,
      protocol_dialect: input.protocolDialect,
      enabled: input.enabled,
    },
  }).then(parseProviderEndpointConfiguration);
}
