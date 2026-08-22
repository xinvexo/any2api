import {
  PROVIDER_KINDS,
  type ProviderKind,
} from "@/shared/api/provider-protocol-vocabulary";

export type { ProviderKind } from "@/shared/api/provider-protocol-vocabulary";

interface BalancingTotals {
  credentialCount: number;
  enabledCredentialCount: number;
  limitedCredentialCount: number;
  rateLimitedCredentialCount: number;
  inFlight: number;
  requestsInWindow: number;
  fixedWaiters: number;
  selected: number;
}

interface BalancingProvider extends BalancingTotals {
  providerKind: ProviderKind;
}

export interface BalancingRuntime {
  configRevision: number;
  schedulerEpoch: number;
  publicRequestsInWindow: number;
  process: {
    activeRequests: number;
    backgroundTasks: number;
    shutdownPhase: "running" | "draining" | "forced";
  };
  transport: {
    cacheEntries: number;
    cacheCapacity: number;
    cacheHits: number;
    cacheMisses: number;
    cacheEvictions: number;
  } | null;
  breakers: {
    closed: number;
    open: number;
    halfOpen: number;
  };
  telemetry: {
    queued: number;
    inFlight: number;
    capacity: number;
    dropped: number;
  };
  queue: {
    waiting: number;
    maxWaiting: number;
    timeoutSecs: number;
    onRateLimited: "wait" | "reject";
    fallbackOnRateLimit: boolean;
  };
  totals: BalancingTotals;
  providers: BalancingProvider[];
}

export function parseBalancingRuntime(value: unknown): BalancingRuntime {
  const root = record(value);
  const process = record(root.process);
  const queue = record(root.queue);
  return {
    configRevision: positive(root.config_revision),
    schedulerEpoch: integer(root.scheduler_epoch),
    publicRequestsInWindow: integer(root.public_requests_in_window),
    process: {
      activeRequests: integer(process.active_requests),
      backgroundTasks: integer(process.background_tasks),
      shutdownPhase: oneOf(process.shutdown_phase, ["running", "draining", "forced"]),
    },
    transport: root.transport === null ? null : parseTransport(root.transport),
    breakers: parseBreakers(root.breakers),
    telemetry: parseTelemetry(root.telemetry),
    queue: {
      waiting: integer(queue.waiting),
      maxWaiting: positive(queue.max_waiting),
      timeoutSecs: positive(queue.timeout_secs),
      onRateLimited: oneOf(queue.on_rate_limited, ["wait", "reject"]),
      fallbackOnRateLimit: boolean(queue.fallback_on_rate_limit),
    },
    totals: parseTotals(root.totals),
    providers: array(root.providers).map(parseProvider),
  };
}

function parseTransport(value: unknown) {
  const item = record(value);
  return {
    cacheEntries: integer(item.cache_entries),
    cacheCapacity: positive(item.cache_capacity),
    cacheHits: integer(item.cache_hits),
    cacheMisses: integer(item.cache_misses),
    cacheEvictions: integer(item.cache_evictions),
  };
}

function parseBreakers(value: unknown) {
  const item = record(value);
  return {
    closed: integer(item.closed),
    open: integer(item.open),
    halfOpen: integer(item.half_open),
  };
}

function parseTelemetry(value: unknown) {
  const item = record(value);
  return {
    queued: integer(item.queued),
    inFlight: integer(item.in_flight),
    capacity: positive(item.capacity),
    dropped: integer(item.dropped),
  };
}

function parseProvider(value: unknown): BalancingProvider {
  const item = record(value);
  return {
    providerKind: oneOf(item.provider_kind, PROVIDER_KINDS),
    ...parseTotals(item),
  };
}

function parseTotals(value: unknown): BalancingTotals {
  const item = record(value);
  return {
    credentialCount: integer(item.credential_count),
    enabledCredentialCount: integer(item.enabled_credential_count),
    limitedCredentialCount: integer(item.limited_credential_count),
    rateLimitedCredentialCount: integer(item.rate_limited_credential_count),
    inFlight: integer(item.in_flight),
    requestsInWindow: integer(item.requests_in_window),
    fixedWaiters: integer(item.fixed_waiters),
    selected: integer(item.selected),
  };
}

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) throw invalid();
  return value as Record<string, unknown>;
}

function array(value: unknown): unknown[] {
  if (!Array.isArray(value)) throw invalid();
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

function invalid() {
  return new Error("invalid balancing runtime response");
}
