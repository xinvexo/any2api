import type { RequestLogFilters } from "../api/request-log-filter-contracts";

export const requestLogQueryKeys = {
  all: ["request-logs"] as const,
  list: (cursor: string | null, pageSize: number, filters: RequestLogFilters = {}) =>
    [
      ...requestLogQueryKeys.all,
      "list",
      cursor ?? "latest",
      pageSize,
      filters.outcome ?? null,
      filters.operation ?? null,
      filters.publicModel ?? null,
      filters.gatewayApiKeyId ?? null,
      filters.credentialId ?? null,
      filters.oauthAccountId ?? null,
    ] as const,
  detail: (requestId: string) => [...requestLogQueryKeys.all, "detail", requestId] as const,
};
