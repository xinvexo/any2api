import { requestJson } from "@/shared/api/http-client";

import {
  parseApplicationAbout,
  parseUpdateCheckResult,
  parseUpdateInstallResult,
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

export function installApplicationUpdate() {
  return requestJson<unknown>("/api/admin/update/install", {
    method: "POST",
    timeoutMs: 360_000,
  }).then(parseUpdateInstallResult);
}
