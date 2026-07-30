import { requestJson } from "@/shared/api/http-client";

import {
  parseApplicationHealthVersion,
  parseApplicationAbout,
  parseUpdateCheckResult,
  parseUpdateStatus,
} from "./update-contracts";

export function getApplicationAbout(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/about", { signal }).then(parseApplicationAbout);
}

export function checkApplicationUpdate() {
  return requestJson<unknown>("/api/admin/update/check", {
    method: "POST",
    timeoutMs: 30_000,
  }).then(parseUpdateCheckResult);
}

export function startApplicationUpdate() {
  return requestJson<unknown>("/api/admin/update/install", {
    method: "POST",
  }).then(parseUpdateStatus);
}

export function getApplicationUpdateStatus() {
  return requestJson<unknown>("/api/admin/update/status", {
    timeoutMs: 2_500,
  }).then(parseUpdateStatus);
}

export function getApplicationHealthVersion() {
  return requestJson<unknown>("/api/health", {
    timeoutMs: 2_500,
  }).then(parseApplicationHealthVersion);
}
