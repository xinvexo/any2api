import type { RuntimeLogLevel } from "@/shared/api/generated/RuntimeLogLevel";
import type { RuntimeLogPage } from "@/shared/api/generated/RuntimeLogPage";
import { requestJson } from "@/shared/api/http-client";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

export interface RuntimeLogFilters {
  level?: RuntimeLogLevel;
  module?: string;
  search?: string;
}

export function getRuntimeLogs(filters: RuntimeLogFilters, cursor: string | null, signal?: AbortSignal) {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(filters)) {
    if (value) query.set(key, value);
  }
  if (cursor) query.set("cursor", cursor);
  return requestJson<RuntimeLogPage>(`${ADMIN_API_PREFIX}/runtime-logs?${query}`, { signal });
}
