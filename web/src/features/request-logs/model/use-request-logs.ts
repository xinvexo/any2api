import { useQuery, useQueryClient } from "@tanstack/react-query";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import { requestLogQueryKeys } from "./request-log-query-keys";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

export function useRequestLogs(cursor: string | null, pageSize: number) {
  const queryClient = useQueryClient();
  const queryKey = requestLogQueryKeys.list(cursor, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getRequestLogs(cursor, pageSize, signal),
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
