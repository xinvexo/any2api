import { requestJson } from "@/shared/api/http-client";

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
): Promise<RequestLogList> {
  return requestJson<unknown>(
    `/api/admin/request-logs?page=${page}&page_size=${pageSize}`,
    { signal },
  ).then(parseRequestLogList);
}

export function getRequestLog(requestId: string, signal?: AbortSignal): Promise<RequestLogDetail> {
  return requestJson<unknown>(
    "/api/admin/request-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
