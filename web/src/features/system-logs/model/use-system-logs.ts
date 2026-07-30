import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { clearSystemLogs, getSystemLogs } from "../api/system-log-api";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (page: number, pageSize: number) => ["system-logs", "list", page, pageSize] as const,
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

export function useClearSystemLogs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: clearSystemLogs,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: systemLogQueryKeys.all });
    },
  });
}
