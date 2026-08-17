import { requestJson } from "@/shared/api/http-client";
import { ADMIN_API_PREFIX } from "@/shared/api/paths";

import {
  parseRequestLogDetail,
  parseRequestLogList,
  type RequestLogDetail,
  type RequestLogList,
} from "./request-log-contracts";
import {
  EMPTY_REQUEST_LOG_FILTERS,
  type RequestLogFilters,
} from "./request-log-filter-contracts";

export function getRequestLogs(
  cursor: string | null = null,
  filters: RequestLogFilters = EMPTY_REQUEST_LOG_FILTERS,
  signal?: AbortSignal,
): Promise<RequestLogList> {
  const query = new URLSearchParams();
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  appendFilter(query, "outcome", filters.outcome);
  appendFilter(query, "public_model", filters.publicModel);
  appendFilter(query, "gateway_api_key_id", filters.gatewayApiKeyId);
  return requestJson<unknown>(
    `${ADMIN_API_PREFIX}/request-logs?${query}`,
    { signal },
  ).then(parseRequestLogList);
}

function appendFilter(query: URLSearchParams, key: string, value?: string) {
  if (value) {
    query.set(key, value);
  }
}

export function getRequestLog(requestId: string, signal?: AbortSignal): Promise<RequestLogDetail> {
  return requestJson<unknown>(
    `${ADMIN_API_PREFIX}/request-logs/` + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
