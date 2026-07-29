import { requestJson } from "@/shared/api/http-client";

import {
  type SettingBatchWriteInput,
  parseSettingsConfiguration,
} from "./settings-contracts";

export function listSettings(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/settings", { signal }).then(parseSettingsConfiguration);
}

export function applySettingChanges(input: SettingBatchWriteInput) {
  return requestJson<unknown>("/api/admin/settings", {
    method: "PATCH",
    body: {
      expected_revision: input.expectedRevision,
      updates: input.updates,
      resets: input.resets,
    },
  }).then(parseSettingsConfiguration);
}
