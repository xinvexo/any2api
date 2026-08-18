import {
  parseProviderCredentialCore,
  type ProviderCredentialCore,
} from "./provider-credential-contracts";

export interface ProviderCredentialMutationResponse {
  configRevision: number;
  providerEndpointId: string;
  items: ProviderCredentialCore[];
}

export function parseProviderCredentialMutationResponse(
  value: unknown,
): ProviderCredentialMutationResponse {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid provider credential mutation response");
  }
  const providerEndpointId = readString(value.provider_endpoint_id);
  const items = value.items.map((item) => {
    if (isRecord(item) && "usage" in item) {
      throw new Error("invalid provider credential mutation response");
    }
    return parseProviderCredentialCore(item);
  });
  if (items.some((item) => item.providerEndpointId !== providerEndpointId)) {
    throw new Error("invalid provider credential mutation response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    providerEndpointId,
    items,
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error("invalid provider credential mutation response");
  }
  return value;
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid provider credential mutation response");
  }
  return Number(value);
}
