type ProviderKind = "codex" | "claude" | "grok";

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
  process: {
    activeRequests: number;
    backgroundTasks: number;
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
    process: {
      activeRequests: integer(process.active_requests),
      backgroundTasks: integer(process.background_tasks),
    },
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

function parseProvider(value: unknown): BalancingProvider {
  const item = record(value);
  return {
    providerKind: oneOf(item.provider_kind, ["codex", "claude", "grok"]),
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
  if (typeof value !== "object" || value === null) throw invalid();
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
