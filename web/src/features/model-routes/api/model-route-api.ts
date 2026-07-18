import { requestJson } from "@/shared/api/http-client";

import {
  type ModelRouteConfiguration,
  type ModelRouteWriteInput,
  type RouteTargetWriteInput,
  parseModelRouteConfiguration,
} from "./model-route-contracts";

const collectionPath = "/api/admin/model-routes";

export function listModelRoutes(signal?: AbortSignal) {
  return requestJson<unknown>(collectionPath, { signal }).then(parseModelRouteConfiguration);
}

export function createModelRoute(input: ModelRouteWriteInput) {
  return writeModelRoute(collectionPath, "POST", input);
}

export function updateModelRoute(id: string, input: ModelRouteWriteInput) {
  return writeModelRoute(`${collectionPath}/${encodeURIComponent(id)}`, "PATCH", input);
}

export function deleteModelRoute(
  id: string,
  expectedRevision: number,
  expectedConfigVersion: number,
) {
  const query = new URLSearchParams({
    expected_revision: String(expectedRevision),
    expected_config_version: String(expectedConfigVersion),
  });
  return requestJson<unknown>(
    `${collectionPath}/${encodeURIComponent(id)}?${query.toString()}`,
    { method: "DELETE" },
  ).then(parseModelRouteConfiguration);
}

function writeModelRoute(
  path: string,
  method: string,
  input: ModelRouteWriteInput,
): Promise<ModelRouteConfiguration> {
  return requestJson<unknown>(path, {
    method,
    body: {
      expected_revision: input.expectedRevision,
      expected_config_version: input.expectedConfigVersion,
      public_model: input.publicModel,
      ingress_protocol: input.ingressProtocol,
      fallback_on_saturation: input.fallbackOnSaturation,
      enabled: input.enabled,
      targets: input.targets.map(writeTarget),
    },
  }).then(parseModelRouteConfiguration);
}

function writeTarget(target: RouteTargetWriteInput) {
  return {
    ...(target.id ? { id: target.id } : {}),
    provider_endpoint_id: target.providerEndpointId,
    upstream_model: target.upstreamModel,
    fallback_tier: target.fallbackTier,
    enabled: target.enabled,
  };
}
