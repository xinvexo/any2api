import { requestJson } from "@/shared/api/http-client";

import {
  parseRequestLogDetail,
  parseRequestLogList,
  type RequestLogDetail,
  type RequestLogList,
} from "./request-log-contracts";

export function getRequestLogs(
  cursor: string | null = null,
  pageSize = 20,
  signal?: AbortSignal,
): Promise<RequestLogList> {
  const query = new URLSearchParams({ page_size: String(pageSize) });
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  return requestJson<unknown>(
    `/api/admin/request-logs?${query}`,
    { signal },
  ).then(parseRequestLogList);
}

export function getRequestLog(requestId: string, signal?: AbortSignal): Promise<RequestLogDetail> {
  return requestJson<unknown>(
    "/api/admin/request-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
