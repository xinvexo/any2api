import { keepPreviousData, useQuery, useQueryClient } from "@tanstack/react-query";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import type { RequestLogFilters } from "../api/request-log-filter-contracts";
import { requestLogQueryKeys } from "./request-log-query-keys";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

export function useRequestLogs(
  cursor: string | null,
  page: number,
  pageSize: number,
  filters: RequestLogFilters,
) {
  const queryClient = useQueryClient();
  const queryKey = requestLogQueryKeys.list(cursor, page, pageSize, filters);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getRequestLogs(cursor, page, pageSize, filters, signal),
    placeholderData: keepPreviousData,
  });

  useLogChangeEvent("request_logs_changed", cursor === null, () => {
    void queryClient.invalidateQueries({ queryKey, exact: true });
  });

  return query;
}

export function useRequestLog(requestId: string) {
  return useQuery({
    queryKey: requestLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getRequestLog(requestId, signal),
    enabled: requestId.length > 0,
  });
}
