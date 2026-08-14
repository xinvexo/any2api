export type CredentialRuntimeStatus =
  | "ready"
  | "disabled"
  | "endpoint_disabled"
  | "authentication_expired"
  | "rate_limited"
  | "proxy_disabled";

export interface CredentialRuntime {
  resolvedProxy: {
    id: string;
    name: string;
    kind: "direct" | "http" | "socks5";
    enabled: boolean;
  };
  rpm60s: {
    used: number;
    limit: number | null;
  };
  inFlight: number;
  status: CredentialRuntimeStatus;
}

export function parseCredentialRuntime(value: unknown, message: string): CredentialRuntime {
  if (!record(value) || !record(value.resolved_proxy) || !record(value.rpm_60s)) {
    throw invalid(message);
  }
  const proxy = value.resolved_proxy;
  const rpm = value.rpm_60s;
  const limit = rpm.limit === null ? null : nonNegativeInteger(rpm.limit, message);
  if (limit === 0) throw invalid(message);
  return {
    resolvedProxy: {
      id: nonEmptyString(proxy.id, message),
      name: nonEmptyString(proxy.name, message),
      kind: oneOf(proxy.kind, ["direct", "http", "socks5"], message),
      enabled: boolean(proxy.enabled, message),
    },
    rpm60s: {
      used: nonNegativeInteger(rpm.used, message),
      limit,
    },
    inFlight: nonNegativeInteger(value.in_flight, message),
    status: oneOf(
      value.status,
      [
        "ready",
        "disabled",
        "endpoint_disabled",
        "authentication_expired",
        "rate_limited",
        "proxy_disabled",
      ],
      message,
    ),
  };
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function nonEmptyString(value: unknown, message: string): string {
  if (typeof value !== "string" || value.length === 0) throw invalid(message);
  return value;
}

function nonNegativeInteger(value: unknown, message: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalid(message);
  }
  return value;
}

function boolean(value: unknown, message: string): boolean {
  if (typeof value !== "boolean") throw invalid(message);
  return value;
}

function oneOf<const T extends string>(value: unknown, values: readonly T[], message: string): T {
  if (typeof value !== "string" || !values.includes(value as T)) throw invalid(message);
  return value as T;
}

function invalid(message: string): Error {
  return new Error(message);
}
