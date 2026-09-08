import type { RuntimeLogPage } from "@/shared/api/generated/RuntimeLogPage";
import { requestJson } from "@/shared/api/http-client";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

export function getRuntimeLogs(cursor: string | null, signal?: AbortSignal) {
  const query = new URLSearchParams();
  if (cursor) query.set("cursor", cursor);
  return requestJson<RuntimeLogPage>(`${ADMIN_API_PREFIX}/runtime-logs?${query}`, { signal });
}
