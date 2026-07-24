export interface UpstreamRequestUsage {
  totalRequests: number;
  successfulRequests: number;
  failedRequests: number;
  recentOutcomes: UpstreamRequestOutcome[];
}

export interface UpstreamRequestOutcome {
  statusCode: number;
}

export function parseUpstreamRequestUsage(value: unknown): UpstreamRequestUsage {
  if (!isRecord(value) || !Array.isArray(value.recent_outcomes)) {
    throw invalidResponse();
  }
  const totalRequests = readCount(value.total_requests);
  const successfulRequests = readCount(value.successful_requests);
  const failedRequests = readCount(value.failed_requests);
  if (
    successfulRequests > totalRequests ||
    failedRequests > totalRequests ||
    successfulRequests + failedRequests !== totalRequests ||
    value.recent_outcomes.length > 24
  ) {
    throw invalidResponse();
  }
  return {
    totalRequests,
    successfulRequests,
    failedRequests,
    recentOutcomes: value.recent_outcomes.map(parseOutcome),
  };
}

function parseOutcome(value: unknown): UpstreamRequestOutcome {
  if (!isRecord(value)) {
    throw invalidResponse();
  }
  const statusCode = readCount(value.status_code);
  if (statusCode < 100 || statusCode > 599) {
    throw invalidResponse();
  }
  return { statusCode };
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
