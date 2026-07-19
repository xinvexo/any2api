import { requestJson } from "@/shared/api/http-client";

import {
  type SettingWriteInput,
  parseSettingsConfiguration,
} from "./settings-contracts";

export function listSettings(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/settings", { signal }).then(parseSettingsConfiguration);
}

export function updateSetting(key: string, input: SettingWriteInput) {
  return requestJson<unknown>(`/api/admin/settings/${encodeURIComponent(key)}`, {
    method: "PATCH",
    body: { expected_revision: input.expectedRevision, value: input.value },
  }).then(parseSettingsConfiguration);
}

export function resetSetting(key: string, expectedRevision: number) {
  return requestJson<unknown>(
    `/api/admin/settings/${encodeURIComponent(key)}?expected_revision=${expectedRevision}`,
    { method: "DELETE" },
  ).then(parseSettingsConfiguration);
}
