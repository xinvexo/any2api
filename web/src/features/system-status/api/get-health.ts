import { getJson } from "@/shared/api/http-client";

export interface HealthResponse {
  status: "ok";
  config_revision: number;
  scheduler_epoch: number;
}

export function getHealth(signal?: AbortSignal) {
  return getJson<unknown>("/api/health", { signal }).then(parseHealthResponse);
}

function parseHealthResponse(value: unknown): HealthResponse {
  if (
    typeof value !== "object" ||
    value === null ||
    !("status" in value) ||
    value.status !== "ok" ||
    !("config_revision" in value) ||
    !Number.isSafeInteger(value.config_revision) ||
    !("scheduler_epoch" in value) ||
    !Number.isSafeInteger(value.scheduler_epoch)
  ) {
    throw new Error("invalid health response");
  }

  return value as HealthResponse;
}
