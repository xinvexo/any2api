export interface OAuthQuotaCredits {
  hasCredits: boolean;
  unlimited: boolean;
  balance: string | null;
}

export type OAuthQuotaReachedType =
  | "rate_limit_reached"
  | "workspace_owner_credits_depleted"
  | "workspace_member_credits_depleted"
  | "workspace_owner_usage_limit_reached"
  | "workspace_member_usage_limit_reached";

export interface OAuthQuotaAccessStatus {
  spendControlReached: boolean | null;
  reachedType: OAuthQuotaReachedType | null;
}

export function parseOAuthQuotaCredits(value: unknown): OAuthQuotaCredits | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    hasCredits: readBoolean(value.has_credits),
    unlimited: readBoolean(value.unlimited),
    balance: readOptionalDecimal(value.balance),
  };
}

export function parseOAuthQuotaAccessStatus(
  value: unknown,
): OAuthQuotaAccessStatus | null {
  if (value === null) return null;
  if (!isRecord(value)) throw invalidResponse();
  return {
    spendControlReached: readOptionalBoolean(value.spend_control_reached),
    reachedType: readReachedType(value.reached_type),
  };
}

function readReachedType(value: unknown): OAuthQuotaReachedType | null {
  if (value === null) return null;
  if (
    value === "rate_limit_reached"
    || value === "workspace_owner_credits_depleted"
    || value === "workspace_member_credits_depleted"
    || value === "workspace_owner_usage_limit_reached"
    || value === "workspace_member_usage_limit_reached"
  ) {
    return value;
  }
  throw invalidResponse();
}

function readOptionalDecimal(value: unknown) {
  if (value === null) return null;
  if (
    typeof value !== "string"
    || value.length > 128
    || !/^\d+(?:\.\d+)?$/.test(value)
  ) {
    throw invalidResponse();
  }
  return value;
}

function readBoolean(value: unknown) {
  if (typeof value !== "boolean") throw invalidResponse();
  return value;
}

function readOptionalBoolean(value: unknown) {
  return value === null ? null : readBoolean(value);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid OAuth quota response");
}
