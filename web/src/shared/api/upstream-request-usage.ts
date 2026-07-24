export interface UpstreamRequestUsage {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  windowMinutes: number;
  windowSlots: UpstreamRequestWindowSlot[];
}

export interface UpstreamRequestWindowSlot {
  startedAtMs: number;
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
}

export function parseUpstreamRequestUsage(value: unknown): UpstreamRequestUsage {
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
    windowMinutes === 0 ||
    value.window_slots.length === 0 ||
    value.window_slots.length > 120
  ) {
    throw invalidResponse();
  }
  return {
    totalRequests,
    successfulRequests,
    failedRequests,
    windowMinutes,
    windowSlots: value.window_slots.map(parseWindowSlot),
  };
}

function parseWindowSlot(value: unknown): UpstreamRequestWindowSlot {
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
  return new Error("invalid upstream request usage response");
}
