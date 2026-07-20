import { useQuery } from "@tanstack/react-query";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import { requestLogQueryKeys } from "./request-log-query-keys";

export function useRequestLogs(limit = 100) {
  return useQuery({
    queryKey: requestLogQueryKeys.list(limit),
    queryFn: ({ signal }) => getRequestLogs(limit, signal),
    refetchInterval: 5_000,
  });
}

export function useRequestLog(requestId: string) {
  return useQuery({
    queryKey: requestLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getRequestLog(requestId, signal),
    enabled: requestId.length > 0,
  });
}
