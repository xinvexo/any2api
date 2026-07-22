export type ProviderKind = "codex" | "claude";
export type ProxyKind = "direct" | "http" | "socks5";
export type HealthStatus = "available" | "cooling" | "unavailable";

export interface HealthState {
  status: HealthStatus;
  retryInMs: number | null;
}

export interface BalancingCounters {
  selectedGeneration: number;
  selectedAuxiliary: number;
  filteredCapacity: number;
  filteredCredentialHealth: number;
  filteredEndpointHealth: number;
  filteredProxyHealth: number;
}

export interface CredentialModelHealth {
  upstreamModel: string;
  credential: HealthState;
  endpoint: HealthState;
  proxy: HealthState;
}

export interface BalancingCredential {
  credentialId: string;
  label: string;
  enabled: boolean;
  providerKind: ProviderKind;
  endpointId: string;
  endpointName: string;
  endpointEnabled: boolean;
  proxyId: string;
  proxyName: string;
  proxyKind: ProxyKind;
  proxyEnabled: boolean;
  inFlight: number;
  maxConcurrency: number;
  fixedWaiters: number;
  auxiliaryInFlight: number;
  counters: BalancingCounters;
  models: CredentialModelHealth[];
}

export interface BalancingRuntime {
  configRevision: number;
  schedulerEpoch: number;
  queue: {
    waiting: number;
    maxWaiting: number;
    timeoutMs: number;
    onSaturated: "wait" | "reject";
    fallbackOnSaturation: boolean;
  };
  auxiliary: { inFlight: number; maxGlobal: number; maxPerCredential: number };
  totals: {
    credentialCount: number;
    enabledCredentialCount: number;
    inFlight: number;
    maxConcurrency: number;
    fixedWaiters: number;
    auxiliaryInFlight: number;
  };
  providers: Array<{
    providerKind: ProviderKind;
    credentialCount: number;
    inFlight: number;
    maxConcurrency: number;
    selectedGeneration: number;
    selectedAuxiliary: number;
  }>;
  credentials: BalancingCredential[];
}

export function parseBalancingRuntime(value: unknown): BalancingRuntime {
  const root = record(value);
  const queue = record(root.queue);
  const auxiliary = record(root.auxiliary);
  const totals = record(root.totals);
  return {
    configRevision: positive(root.config_revision),
    schedulerEpoch: integer(root.scheduler_epoch),
    queue: {
      waiting: integer(queue.waiting),
      maxWaiting: positive(queue.max_waiting),
      timeoutMs: positive(queue.timeout_ms),
      onSaturated: oneOf(queue.on_saturated, ["wait", "reject"]),
      fallbackOnSaturation: boolean(queue.fallback_on_saturation),
    },
    auxiliary: {
      inFlight: integer(auxiliary.in_flight),
      maxGlobal: positive(auxiliary.max_global),
      maxPerCredential: positive(auxiliary.max_per_credential),
    },
    totals: {
      credentialCount: integer(totals.credential_count),
      enabledCredentialCount: integer(totals.enabled_credential_count),
      inFlight: integer(totals.in_flight),
      maxConcurrency: integer(totals.max_concurrency),
      fixedWaiters: integer(totals.fixed_waiters),
      auxiliaryInFlight: integer(totals.auxiliary_in_flight),
    },
    providers: array(root.providers).map(parseProvider),
    credentials: array(root.credentials).map(parseCredential),
  };
}

function parseProvider(value: unknown) {
  const item = record(value);
  return {
    providerKind: provider(item.provider_kind),
    credentialCount: integer(item.credential_count),
    inFlight: integer(item.in_flight),
    maxConcurrency: integer(item.max_concurrency),
    selectedGeneration: integer(item.selected_generation),
    selectedAuxiliary: integer(item.selected_auxiliary),
  };
}

function parseCredential(value: unknown): BalancingCredential {
  const item = record(value);
  return {
    credentialId: string(item.credential_id),
    label: string(item.label),
    enabled: boolean(item.enabled),
    providerKind: provider(item.provider_kind),
    endpointId: string(item.endpoint_id),
    endpointName: string(item.endpoint_name),
    endpointEnabled: boolean(item.endpoint_enabled),
    proxyId: string(item.proxy_id),
    proxyName: string(item.proxy_name),
    proxyKind: oneOf(item.proxy_kind, ["direct", "http", "socks5"]),
    proxyEnabled: boolean(item.proxy_enabled),
    inFlight: integer(item.in_flight),
    maxConcurrency: positive(item.max_concurrency),
    fixedWaiters: integer(item.fixed_waiters),
    auxiliaryInFlight: integer(item.auxiliary_in_flight),
    counters: parseCounters(item.counters),
    models: array(item.models).map(parseModel),
  };
}

function parseCounters(value: unknown): BalancingCounters {
  const item = record(value);
  return {
    selectedGeneration: integer(item.selected_generation),
    selectedAuxiliary: integer(item.selected_auxiliary),
    filteredCapacity: integer(item.filtered_capacity),
    filteredCredentialHealth: integer(item.filtered_credential_health),
    filteredEndpointHealth: integer(item.filtered_endpoint_health),
    filteredProxyHealth: integer(item.filtered_proxy_health),
  };
}

function parseModel(value: unknown): CredentialModelHealth {
  const item = record(value);
  return {
    upstreamModel: string(item.upstream_model),
    credential: parseHealth(item.credential),
    endpoint: parseHealth(item.endpoint),
    proxy: parseHealth(item.proxy),
  };
}

function parseHealth(value: unknown): HealthState {
  const item = record(value);
  const status = oneOf(item.status, ["available", "cooling", "unavailable"]);
  const retryInMs = item.retry_in_ms === null ? null : positive(item.retry_in_ms);
  if ((status === "cooling") !== (retryInMs !== null)) throw invalid();
  return { status, retryInMs };
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null) throw invalid();
  return value as Record<string, unknown>;
}

function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) throw invalid();
  return value;
}

function string(value: unknown): string {
  if (typeof value !== "string" || value.length === 0) throw invalid();
  return value;
}

function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") throw invalid();
  return value;
}

function integer(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) throw invalid();
  return value;
}

function positive(value: unknown): number {
  const result = integer(value);
  if (result === 0) throw invalid();
  return result;
}

function oneOf<const T extends string>(value: unknown, values: readonly T[]): T {
  if (typeof value !== "string" || !values.includes(value as T)) throw invalid();
  return value as T;
}

function provider(value: unknown) {
  return oneOf(value, ["codex", "claude"]);
}

function invalid() {
  return new Error("invalid balancing runtime response");
}
