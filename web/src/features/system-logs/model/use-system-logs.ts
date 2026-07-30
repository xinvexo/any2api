import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { clearSystemLogs, getSystemLogs } from "../api/system-log-api";

const systemLogQueryKeys = {
  all: ["system-logs"] as const,
  list: (page: number, pageSize: number) => ["system-logs", "list", page, pageSize] as const,
};
const AUTO_REFRESH_INTERVAL_MS = 5_000;

export function useSystemLogs(autoRefresh: boolean, page: number, pageSize: number) {
  const queryClient = useQueryClient();
  const pageKey = `${page}:${pageSize}`;
  const [automaticErrorPage, setAutomaticErrorPage] = useState<string | null>(null);
  const queryKey = systemLogQueryKeys.list(page, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getSystemLogs(page, pageSize, signal),
  });

  useEffect(() => {
    if (!autoRefresh) {
      return;
    }

    let controller: AbortController | null = null;
    let active = true;
    const refresh = () => {
      if (controller) {
        return;
      }
      controller = new AbortController();
      setAutomaticErrorPage(null);
      void getSystemLogs(page, pageSize, controller.signal, "automatic")
        .then((data) => {
          if (active) {
            queryClient.setQueryData(systemLogQueryKeys.list(page, pageSize), data);
            setAutomaticErrorPage(null);
          }
        })
        .catch((error: unknown) => {
          if (active && !isAbortError(error)) {
            setAutomaticErrorPage(pageKey);
          }
        })
        .finally(() => {
          controller = null;
        });
    };
    const interval = window.setInterval(refresh, AUTO_REFRESH_INTERVAL_MS);

    return () => {
      active = false;
      window.clearInterval(interval);
      controller?.abort();
    };
  }, [autoRefresh, page, pageKey, pageSize, queryClient]);

  const refetch: typeof query.refetch = (options) => {
    setAutomaticErrorPage(null);
    return query.refetch(options);
  };

  return {
    ...query,
    isError: query.isError || (autoRefresh && automaticErrorPage === pageKey),
    refetch,
  };
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

function isAbortError(error: unknown) {
  return (
    typeof error === "object"
    && error !== null
    && "name" in error
    && error.name === "AbortError"
  );
}
