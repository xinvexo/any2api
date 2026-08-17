import { useQuery } from "@tanstack/react-query";

import { getSystemLog } from "../api/system-log-api";
import { systemLogQueryKeys } from "./system-log-query-keys";

export function useSystemLog(requestId: string) {
  return useQuery({
    queryKey: systemLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getSystemLog(requestId, signal),
    enabled: requestId.length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  });
}
