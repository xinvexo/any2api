import type { RequestLogFilters } from "../api/request-log-filter-contracts";

export const requestLogQueryKeys = {
  all: ["request-logs"] as const,
  list: (filters: RequestLogFilters = {}) =>
    [
      ...requestLogQueryKeys.all,
      "list",
      filters.outcome ?? null,
      filters.publicModel ?? null,
      filters.gatewayApiKeyId ?? null,
    ] as const,
  detail: (requestId: string) => [...requestLogQueryKeys.all, "detail", requestId] as const,
};
