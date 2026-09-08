import { useCallback } from "react";

import { getSystemLogs } from "../api/system-log-api";
import { EMPTY_SYSTEM_LOG_FILTERS, type SystemLog, type SystemLogFilters, type SystemLogList } from "../api/system-log-contracts";
import {
  MAX_CACHED_SYSTEM_BATCHES,
  MAX_CACHED_SYSTEM_LOGS,
  countNewSystemLogs,
  flattenSystemLogPages,
  mergeLatestSystemBatches,
  systemLogIds,
} from "./system-log-feed";
import { currentSystemLogFeedGeneration } from "./system-log-feed-generation";
import { systemLogQueryKeys } from "./system-log-query-keys";
import { useAdminEvent } from "@/shared/realtime";
import { useRealtimeCursorFeed } from "@/shared/realtime/use-realtime-cursor-feed";

export function useSystemLogs(
  showAdminOperations: boolean,
  followingLatest: boolean,
  filters: SystemLogFilters = EMPTY_SYSTEM_LOG_FILTERS,
) {
  const fetchPage = useCallback(
    (cursor: string | null, signal?: AbortSignal) =>
      getSystemLogs(showAdminOperations, cursor, signal, filters),
    [showAdminOperations, filters],
  );
  const feed = useRealtimeCursorFeed<SystemLog, SystemLogList, SystemLogFeed>({
    queryKey: systemLogQueryKeys.list(showAdminOperations, filters),
    scope: JSON.stringify([showAdminOperations, filters]),
    followingLatest,
    fetchPage,
    knownIds: systemLogIds,
    itemId: systemLogId,
    mergeLatest: mergeLatestSystemBatches,
    countNew: countNewSystemLogs,
    flatten: flattenSystemLogPages,
    maxCachedPages: MAX_CACHED_SYSTEM_BATCHES,
    maxCollectedItems: MAX_CACHED_SYSTEM_LOGS,
    currentExternalGeneration: currentSystemLogFeedGeneration,
    syncErrorMessage: "系统日志同步失败",
  });

  useAdminEvent("system_logs_changed", true, feed.scheduleSync);
  return feed;
}

type SystemLogFeed = ReturnType<typeof flattenSystemLogPages>;

function systemLogId(item: SystemLog) {
  return item.requestId;
}
