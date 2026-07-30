import { useQuery, useQueryClient } from "@tanstack/react-query";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import { requestLogQueryKeys } from "./request-log-query-keys";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

export function useRequestLogs(page: number, pageSize: number) {
  const queryClient = useQueryClient();
  const queryKey = requestLogQueryKeys.list(page, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getRequestLogs(page, pageSize, signal),
  });

  useLogChangeEvent("request_logs_changed", true, () => {
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
