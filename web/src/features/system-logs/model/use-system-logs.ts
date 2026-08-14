import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { clearSystemLogs, getSystemLog, getSystemLogs } from "../api/system-log-api";
import { useLogChangeEvent } from "@/shared/lib/use-log-change-event";

const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (cursor: string | null, page: number, pageSize: number) =>
    ["system-logs", "list", cursor ?? "latest", page, pageSize] as const,
  detail: (requestId: string) => ["system-logs", "detail", requestId] as const,
};

export function useSystemLogs(
  autoRefresh: boolean,
  cursor: string | null,
  page: number,
  pageSize: number,
) {
  const queryClient = useQueryClient();
  const queryKey = systemLogQueryKeys.list(cursor, page, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getSystemLogs(cursor, page, pageSize, signal),
    placeholderData: keepPreviousData,
  });

  useLogChangeEvent("system_logs_changed", autoRefresh && cursor === null, () => {
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
