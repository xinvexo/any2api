import { requestJson } from "@/shared/api/http-client";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

import {
  parseClearSystemLogsResult,
  parseSystemLogDetail,
  parseSystemLogList,
  type ClearSystemLogsResult,
  type SystemLogDetail,
  type SystemLogList,
  type SystemLogFilters,
  EMPTY_SYSTEM_LOG_FILTERS,
} from "./system-log-contracts";

export function getSystemLogs(
  showAdminOperations = true,
  cursor: string | null = null,
  signal?: AbortSignal,
  filters: SystemLogFilters = EMPTY_SYSTEM_LOG_FILTERS,
): Promise<SystemLogList> {
  const query = new URLSearchParams({
    show_admin_operations: String(showAdminOperations),
  });
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  if (filters.statusCode) query.set("status_code", filters.statusCode);
  if (filters.clientIp) query.set("client_ip", filters.clientIp);
  if (filters.path) query.set("path", filters.path);
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
