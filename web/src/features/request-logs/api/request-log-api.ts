import { requestJson } from "@/shared/api/http-client";

import {
  parseRequestLogDetail,
  parseRequestLogList,
  type RequestLogDetail,
  type RequestLogList,
} from "./request-log-contracts";

export function getRequestLogs(limit = 100, signal?: AbortSignal): Promise<RequestLogList> {
  return requestJson<unknown>("/api/admin/request-logs?limit=" + limit, { signal }).then(
    parseRequestLogList,
  );
}

export function getRequestLog(requestId: string, signal?: AbortSignal): Promise<RequestLogDetail> {
  return requestJson<unknown>(
    "/api/admin/request-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
