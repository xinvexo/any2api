import { getJson } from "@/shared/api/http-client";

export interface HealthResponse {
  status: "ok";
  config_revision: number;
  scheduler_epoch: number;
  shutdown_phase: "running" | "draining" | "forced";
  active_requests: number;
  background_tasks: number;
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
    !Number.isSafeInteger(value.scheduler_epoch) ||
    !("shutdown_phase" in value) ||
    (value.shutdown_phase !== "running" && value.shutdown_phase !== "draining" && value.shutdown_phase !== "forced") ||
    !("active_requests" in value) ||
    !isNonNegativeInteger(value.active_requests) ||
    !("background_tasks" in value) ||
    !isNonNegativeInteger(value.background_tasks)
  ) {
    throw new Error("invalid health response");
  }

  return value as HealthResponse;
}

function isNonNegativeInteger(value: unknown) {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}
