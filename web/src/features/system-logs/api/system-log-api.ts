import { requestJson } from "@/shared/api/http-client";

import {
  parseClearSystemLogsResult,
  parseSystemLogDetail,
  parseSystemLogList,
  type ClearSystemLogsResult,
  type SystemLogDetail,
  type SystemLogList,
} from "./system-log-contracts";

export function getSystemLogs(
  cursor: string | null = null,
  pageSize = 20,
  signal?: AbortSignal,
): Promise<SystemLogList> {
  const query = new URLSearchParams({ page_size: String(pageSize) });
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  return requestJson<unknown>(`/api/admin/system-logs?${query}`, {
    signal,
  }).then(parseSystemLogList);
}

export function getSystemLog(
  requestId: string,
  signal?: AbortSignal,
): Promise<SystemLogDetail> {
  return requestJson<unknown>(
    "/api/admin/system-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseSystemLogDetail);
}

export function clearSystemLogs(): Promise<ClearSystemLogsResult> {
  return requestJson<unknown>("/api/admin/system-logs", { method: "DELETE" }).then(
    parseClearSystemLogsResult,
  );
}
