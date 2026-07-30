import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import { requestLogQueryKeys } from "./request-log-query-keys";

const AUTO_REFRESH_INTERVAL_MS = 1_000;

export function useRequestLogs(page: number, pageSize: number) {
  const queryClient = useQueryClient();
  const pageKey = `${page}:${pageSize}`;
  const [automaticErrorPage, setAutomaticErrorPage] = useState<string | null>(null);
  const queryKey = requestLogQueryKeys.list(page, pageSize);
  const query = useQuery({
    queryKey,
    queryFn: ({ signal }) => getRequestLogs(page, pageSize, signal),
  });

  useEffect(() => {
    let controller: AbortController | null = null;
    let active = true;
    const refresh = () => {
      if (controller) {
        return;
      }
      controller = new AbortController();
      void getRequestLogs(page, pageSize, controller.signal, "automatic")
        .then((data) => {
          if (active) {
            queryClient.setQueryData(requestLogQueryKeys.list(page, pageSize), data);
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
  }, [page, pageKey, pageSize, queryClient]);

  const refetch: typeof query.refetch = (options) => {
    setAutomaticErrorPage(null);
    return query.refetch(options);
  };

  return {
    ...query,
    isError: query.isError || automaticErrorPage === pageKey,
    refetch,
  };
}

export function useRequestLog(requestId: string) {
  return useQuery({
    queryKey: requestLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getRequestLog(requestId, signal),
    enabled: requestId.length > 0,
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
