import {
  parseOAuthAccountCore,
  type OAuthAccountCore,
} from "./oauth-contracts";

export interface OAuthAccountMutationResponse {
  configRevision: number;
  items: OAuthAccountCore[];
}

export function parseOAuthAccountMutationResponse(
  value: unknown,
): OAuthAccountMutationResponse {
  if (!isRecord(value) || !Array.isArray(value.items)) {
    throw new Error("invalid OAuth account mutation response");
  }
  return {
    configRevision: readPositiveInteger(value.config_revision),
    items: value.items.map((item) => {
      if (isRecord(item) && ("available_models" in item || "usage" in item)) {
        throw new Error("invalid OAuth account mutation response");
      }
      return parseOAuthAccountCore(item);
    }),
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readPositiveInteger(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) <= 0) {
    throw new Error("invalid OAuth account mutation response");
  }
  return Number(value);
}
