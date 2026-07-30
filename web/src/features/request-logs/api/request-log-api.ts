import { requestJson } from "@/shared/api/http-client";
import { AUTOMATIC_LOG_REFRESH_HEADERS } from "@/shared/api/log-refresh";

import {
  parseRequestLogDetail,
  parseRequestLogList,
  type RequestLogDetail,
  type RequestLogList,
} from "./request-log-contracts";

export function getRequestLogs(
  page = 1,
  pageSize = 20,
  signal?: AbortSignal,
  refreshKind: "ordinary" | "automatic" = "ordinary",
): Promise<RequestLogList> {
  return requestJson<unknown>(
    `/api/admin/request-logs?page=${page}&page_size=${pageSize}`,
    {
      signal,
      headers: refreshKind === "automatic" ? AUTOMATIC_LOG_REFRESH_HEADERS : undefined,
    },
  ).then(parseRequestLogList);
}

export function getRequestLog(requestId: string, signal?: AbortSignal): Promise<RequestLogDetail> {
  return requestJson<unknown>(
    "/api/admin/request-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
