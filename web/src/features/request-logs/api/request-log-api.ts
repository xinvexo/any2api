import { requestJson } from "@/shared/api/http-client";

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
  pageSize = 20,
  filters: RequestLogFilters = EMPTY_REQUEST_LOG_FILTERS,
  signal?: AbortSignal,
): Promise<RequestLogList> {
  if (filters.credentialId && filters.oauthAccountId) {
    throw new Error("request log upstream filters are mutually exclusive");
  }
  const query = new URLSearchParams({ page_size: String(pageSize) });
  if (cursor !== null) {
    query.set("cursor", cursor);
  }
  appendFilter(query, "outcome", filters.outcome);
  appendFilter(query, "operation", filters.operation);
  appendFilter(query, "public_model", filters.publicModel);
  appendFilter(query, "gateway_api_key_id", filters.gatewayApiKeyId);
  appendFilter(query, "credential_id", filters.credentialId);
  appendFilter(query, "oauth_account_id", filters.oauthAccountId);
  return requestJson<unknown>(
    `/api/admin/request-logs?${query}`,
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
    "/api/admin/request-logs/" + encodeURIComponent(requestId),
    { signal },
  ).then(parseRequestLogDetail);
}
