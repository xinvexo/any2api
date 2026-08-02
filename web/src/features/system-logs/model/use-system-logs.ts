import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { clearSystemLogs, getSystemLog, getSystemLogs } from "../api/system-log-api";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (page: number, pageSize: number) => ["system-logs", "list", page, pageSize] as const,
  detail: (requestId: string) => ["system-logs", "detail", requestId] as const,
};

export function useSystemLogs(autoRefresh: boolean, page: number, pageSize: number) {
  const queryClient = useQueryClient();
  const queryKey = systemLogQueryKeys.list(page, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getSystemLogs(page, pageSize, signal),
  });

  useLogChangeEvent("system_logs_changed", autoRefresh, () => {
    void queryClient.invalidateQueries({ queryKey, exact: true });
  });

  return query;
}

export function useSystemLog(requestId: string) {
  return useQuery({
    queryKey: systemLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getSystemLog(requestId, signal),
    enabled: requestId.length > 0,
  });
}

export function useClearSystemLogs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: clearSystemLogs,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: systemLogQueryKeys.all });
    },
  });
}
