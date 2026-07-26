export const REQUEST_USAGE_WINDOW_MINUTES = 2;
export const REQUEST_USAGE_WINDOW_SLOT_COUNT = 30;

export interface RequestUsage {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  windowMinutes: number;
  windowSlots: RequestUsageWindowSlot[];
}

export interface RequestUsageWindowSlot {
  startedAtMs: number;
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
}

export function parseRequestUsage(value: unknown): RequestUsage {
  if (!isRecord(value) || !Array.isArray(value.window_slots)) {
    throw invalidResponse();
  }
  const totalRequests = readCount(value.total_requests);
  const successfulRequests = readCount(value.successful_requests);
  const failedRequests = readCount(value.failed_requests);
  const windowMinutes = readCount(value.window_minutes);
  if (
    successfulRequests > totalRequests ||
    failedRequests > totalRequests ||
    successfulRequests + failedRequests !== totalRequests ||
    windowMinutes !== REQUEST_USAGE_WINDOW_MINUTES ||
    value.window_slots.length !== REQUEST_USAGE_WINDOW_SLOT_COUNT
  ) {
    throw invalidResponse();
  }

  const windowSlots = value.window_slots.map(parseWindowSlot);
  const intervalMs = windowMinutes * 60_000;
  if (
    windowSlots.some(
      (slot, index) => index > 0 && slot.startedAtMs !== windowSlots[index - 1].startedAtMs + intervalMs,
    ) ||
    windowSlots.reduce((sum, slot) => sum + slot.totalRequests, 0) > totalRequests
  ) {
    throw invalidResponse();
  }
  return {
    totalRequests,
    successfulRequests,
    failedRequests,
    windowMinutes,
    windowSlots,
  };
}

function parseWindowSlot(value: unknown): RequestUsageWindowSlot {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const totalRequests = readCount(value.total_requests);
  const successfulRequests = readCount(value.successful_requests);
  const failedRequests = readCount(value.failed_requests);
  if (
    successfulRequests > totalRequests ||
    failedRequests > totalRequests ||
    successfulRequests + failedRequests !== totalRequests
  ) {
    throw invalidResponse();
  }
  return {
    startedAtMs: readCount(value.started_at_ms),
    totalRequests,
    successfulRequests,
    failedRequests,
  };
}

function readCount(value: unknown) {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw invalidResponse();
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function invalidResponse() {
  return new Error("invalid request usage response");
}
