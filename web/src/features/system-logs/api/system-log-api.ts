import { requestJson } from "@/shared/api/http-client";
import { AUTOMATIC_LOG_REFRESH_HEADERS } from "@/shared/api/log-refresh";

import {
  parseClearSystemLogsResult,
  parseSystemLogList,
  type ClearSystemLogsResult,
  type SystemLogList,
} from "./system-log-contracts";

export function getSystemLogs(
  page = 1,
  pageSize = 20,
  signal?: AbortSignal,
  refreshKind: "ordinary" | "automatic" = "ordinary",
): Promise<SystemLogList> {
  return requestJson<unknown>(`/api/admin/system-logs?page=${page}&page_size=${pageSize}`, {
    signal,
    headers: refreshKind === "automatic" ? AUTOMATIC_LOG_REFRESH_HEADERS : undefined,
  }).then(parseSystemLogList);
}

export function clearSystemLogs(): Promise<ClearSystemLogsResult> {
  return requestJson<unknown>("/api/admin/system-logs", { method: "DELETE" }).then(
    parseClearSystemLogsResult,
  );
}
