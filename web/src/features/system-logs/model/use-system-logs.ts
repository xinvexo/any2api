import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { clearSystemLogs, getSystemLogs } from "../api/system-log-api";

const systemLogQueryKey = ["system-logs", 200] as const;
const AUTO_REFRESH_INTERVAL_MS = 5_000;

export function useSystemLogs(autoRefresh: boolean) {
  const queryClient = useQueryClient();
  const [automaticFetching, setAutomaticFetching] = useState(false);
  const [automaticError, setAutomaticError] = useState(false);
  const query = useQuery({
    queryKey: systemLogQueryKey,
    queryFn: ({ signal }) => getSystemLogs(200, signal),
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
      setAutomaticFetching(true);
      setAutomaticError(false);
      void getSystemLogs(200, controller.signal, "automatic")
        .then((data) => {
          if (active) {
            queryClient.setQueryData(systemLogQueryKey, data);
            setAutomaticError(false);
          }
        })
        .catch((error: unknown) => {
          if (active && !isAbortError(error)) {
            setAutomaticError(true);
          }
        })
        .finally(() => {
          controller = null;
          if (active) {
            setAutomaticFetching(false);
          }
        });
    };
    const interval = window.setInterval(refresh, AUTO_REFRESH_INTERVAL_MS);

    return () => {
      active = false;
      window.clearInterval(interval);
      controller?.abort();
    };
  }, [autoRefresh, queryClient]);

  const refetch: typeof query.refetch = (options) => {
    setAutomaticError(false);
    return query.refetch(options);
  };

  return {
    ...query,
    isFetching: query.isFetching || (autoRefresh && automaticFetching),
    isError: query.isError || (autoRefresh && automaticError),
    refetch,
  };
}

export function useClearSystemLogs() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: clearSystemLogs,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: systemLogQueryKey });
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
