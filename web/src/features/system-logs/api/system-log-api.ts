import { requestJson } from "@/shared/api/http-client";

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
): Promise<SystemLogList> {
  return requestJson<unknown>(`/api/admin/system-logs?page=${page}&page_size=${pageSize}`, {
    signal,
  }).then(parseSystemLogList);
}

export function clearSystemLogs(): Promise<ClearSystemLogsResult> {
  return requestJson<unknown>("/api/admin/system-logs", { method: "DELETE" }).then(
    parseClearSystemLogsResult,
  );
}
