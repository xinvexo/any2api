import { requestJson } from "@/shared/api/http-client";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

import {
  parseClearSystemLogsResult,
  parseSystemLogDetail,
  parseSystemLogList,
  type ClearSystemLogsResult,
  type SystemLogDetail,
  type SystemLogList,
} from "./system-log-contracts";

export function getSystemLogs(
  showAdminOperations = true,
  cursor: string | null = null,
  signal?: AbortSignal,
): Promise<SystemLogList> {
  const query = new URLSearchParams({
    show_admin_operations: String(showAdminOperations),
  });
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  return requestJson<unknown>(`${ADMIN_API_PREFIX}/system-logs?${query}`, {
    signal,
  }).then(parseSystemLogList);
}

export function getSystemLog(
  requestId: string,
  signal?: AbortSignal,
): Promise<SystemLogDetail> {
  return requestJson<unknown>(
    `${ADMIN_API_PREFIX}/system-logs/` + encodeURIComponent(requestId),
    { signal },
  ).then(parseSystemLogDetail);
}

export function clearSystemLogs(): Promise<ClearSystemLogsResult> {
  return requestJson<unknown>(`${ADMIN_API_PREFIX}/system-logs`, { method: "DELETE" }).then(
    parseClearSystemLogsResult,
  );
}
