import { requestJson } from "@/shared/api/http-client";

import {
  parseClearSystemLogsResult,
  parseSystemLogList,
  type ClearSystemLogsResult,
  type SystemLogList,
} from "./system-log-contracts";

const AUTOMATIC_REFRESH_HEADER = "X-Any2API-System-Log-Refresh";

export function getSystemLogs(
  limit = 200,
  signal?: AbortSignal,
  refreshKind: "ordinary" | "automatic" = "ordinary",
): Promise<SystemLogList> {
  return requestJson<unknown>(`/api/admin/system-logs?limit=${limit}`, {
    signal,
    headers:
      refreshKind === "automatic"
        ? { [AUTOMATIC_REFRESH_HEADER]: "automatic" }
        : undefined,
  }).then(parseSystemLogList);
}

export function clearSystemLogs(): Promise<ClearSystemLogsResult> {
  return requestJson<unknown>("/api/admin/system-logs", { method: "DELETE" }).then(
    parseClearSystemLogsResult,
  );
}
