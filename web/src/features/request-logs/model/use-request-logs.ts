import { useQuery } from "@tanstack/react-query";
import { useCallback } from "react";

import { getRequestLog, getRequestLogs } from "../api/request-log-api";
import type { RequestLog, RequestLogList } from "../api/request-log-contracts";
import type { RequestLogFilters } from "../api/request-log-filter-contracts";
import {
  MAX_CACHED_REQUEST_BATCHES,
  MAX_CACHED_REQUEST_LOGS,
  completedRequestLogIds,
  countNewRequestLogs,
  flattenRequestLogPages,
  mergeLatestRequestBatches,
} from "./request-log-feed";
import { requestLogQueryKeys } from "./request-log-query-keys";
import { useAdminEvent } from "@/shared/realtime";
import { useRealtimeCursorFeed } from "@/shared/realtime/use-realtime-cursor-feed";

export function useRequestLogs(filters: RequestLogFilters, followingLatest: boolean) {
  const scope = `${filters.outcome ?? ""}\u0000${filters.publicModel ?? ""}\u0000${filters.gatewayApiKeyId ?? ""}`;
  const fetchPage = useCallback(
    (cursor: string | null, signal?: AbortSignal) =>
      getRequestLogs(cursor, filters, signal),
    [filters],
  );
  const feed = useRealtimeCursorFeed<RequestLog, RequestLogList, RequestLogFeed>({
    queryKey: requestLogQueryKeys.list(filters),
    scope,
    followingLatest,
    fetchPage,
    knownIds: completedRequestLogIds,
    itemId: requestLogId,
    mergeLatest: mergeLatestRequestBatches,
    countNew: countNewRequestLogs,
    flatten: flattenRequestLogPages,
    maxCachedPages: MAX_CACHED_REQUEST_BATCHES,
    maxCollectedItems: MAX_CACHED_REQUEST_LOGS - 100,
    syncErrorMessage: "请求日志同步失败",
  });

  useAdminEvent("request_logs_changed", true, feed.scheduleSync);
  useAdminEvent("active_requests_changed", true, feed.scheduleSync);
  return feed;
}

export function useRequestLog(requestId: string) {
  return useQuery({
    queryKey: requestLogQueryKeys.detail(requestId),
    queryFn: ({ signal }) => getRequestLog(requestId, signal),
    enabled: requestId.length > 0,
    staleTime: 30_000,
    gcTime: 5 * 60_000,
  });
}

type RequestLogFeed = ReturnType<typeof flattenRequestLogPages>;

function requestLogId(item: RequestLog) {
  return item.requestId;
}
