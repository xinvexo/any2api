import {
  useInfiniteQuery,
  useQuery,
  useQueryClient,
  type InfiniteData,
} from "@tanstack/react-query";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

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
import { collectCursorBatches } from "@/shared/lib/collect-cursor-batches";
import { useAdminEvent } from "@/shared/realtime";

const EVENT_BATCH_MS = 100;

interface PendingLatest {
  scope: string;
  count: number;
}

interface SyncFailure {
  scope: string;
  error: Error;
}

export function useRequestLogs(filters: RequestLogFilters, followingLatest: boolean) {
  const queryClient = useQueryClient();
  const queryKey = requestLogQueryKeys.list(filters);
  const scope = `${filters.outcome ?? ""}\u0000${filters.publicModel ?? ""}\u0000${filters.gatewayApiKeyId ?? ""}`;
  const followingRef = useRef(followingLatest);
  const scopeRef = useRef(scope);
  const timerRef = useRef<number | null>(null);
  const eventCountRef = useRef(0);
  const syncingRef = useRef(false);
  const refreshingRef = useRef(false);
  const rerunRef = useRef(false);
  const resetOnNextSyncRef = useRef(false);
  const generationRef = useRef(0);
  const runSyncRef = useRef<() => Promise<void>>(async () => undefined);
  const [pending, setPending] = useState<PendingLatest | null>(null);
  const [syncFailure, setSyncFailure] = useState<SyncFailure | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);

  useLayoutEffect(() => {
    if (scopeRef.current !== scope) {
      scopeRef.current = scope;
      generationRef.current += 1;
      resetOnNextSyncRef.current = false;
      queryClient.removeQueries({ queryKey, exact: true });
      setPending(null);
      setSyncFailure(null);
    }
  }, [queryClient, queryKey, scope]);

  const query = useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam, signal }) => getRequestLogs(pageParam, filters, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    maxPages: MAX_CACHED_REQUEST_BATCHES,
    staleTime: Number.POSITIVE_INFINITY,
  });

  const applyLatest = useCallback(
    (latest: InfiniteData<RequestLogList, string | null>) => {
      queryClient.setQueryData<InfiniteData<RequestLogList, string | null>>(
        queryKey,
        (current) => mergeLatestRequestBatches(current, latest),
      );
      setPending(null);
    },
    [queryClient, queryKey],
  );

  const syncLatest = useCallback(async () => {
    const generation = generationRef.current;
    const current = queryClient.getQueryData<InfiniteData<RequestLogList, string | null>>(
      queryKey,
    );
    if (!current) {
      return;
    }
    const latest = await collectCursorBatches<RequestLog, RequestLogList>(
      (cursor) => getRequestLogs(cursor, filters),
      completedRequestLogIds(current),
      (item) => item.requestId,
      MAX_CACHED_REQUEST_BATCHES,
      MAX_CACHED_REQUEST_LOGS - 100,
    );
    if (scopeRef.current !== scope || generationRef.current !== generation) {
      return;
    }
    if (followingRef.current) {
      if (resetOnNextSyncRef.current) {
        resetOnNextSyncRef.current = false;
        queryClient.setQueryData(queryKey, latest);
        setPending(null);
      } else {
        applyLatest(latest);
      }
      return;
    }
    setPending({
      scope,
      count: countNewRequestLogs(current, latest),
    });
  }, [applyLatest, filters, queryClient, queryKey, scope]);

  const queueSyncRun = useCallback(() => {
    if (timerRef.current !== null) return;
    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      void runSyncRef.current();
    }, EVENT_BATCH_MS);
  }, []);

  const runSync = useCallback(async () => {
    if (syncingRef.current || refreshingRef.current) {
      rerunRef.current = true;
      return;
    }
    syncingRef.current = true;
    const generation = generationRef.current;
    eventCountRef.current = 0;
    try {
      await syncLatest();
      if (scopeRef.current === scope && generationRef.current === generation) {
        setSyncFailure(null);
      }
    } catch (error) {
      if (scopeRef.current === scope && generationRef.current === generation) {
        setSyncFailure({ scope, error: asError(error) });
      }
    } finally {
      syncingRef.current = false;
      if (rerunRef.current || eventCountRef.current > 0) {
        rerunRef.current = false;
        queueSyncRun();
      }
    }
  }, [queueSyncRun, scope, syncLatest]);

  useLayoutEffect(() => {
    runSyncRef.current = runSync;
  }, [runSync]);

  const scheduleSync = useCallback(() => {
    eventCountRef.current += 1;
    if (syncingRef.current || refreshingRef.current) {
      rerunRef.current = true;
      return;
    }
    queueSyncRun();
  }, [queueSyncRun]);

  useAdminEvent("request_logs_changed", true, scheduleSync);
  useAdminEvent("active_requests_changed", true, scheduleSync);

  useEffect(() => {
    scheduleSync();
  }, [scheduleSync, scope]);

  useEffect(() => {
    followingRef.current = followingLatest;
    if (followingLatest && pending?.scope === scope && pending.count > 0) {
      resetOnNextSyncRef.current = true;
      scheduleSync();
    }
  }, [followingLatest, pending, scheduleSync, scope]);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      eventCountRef.current = 0;
      rerunRef.current = false;
    };
  }, [scope]);

  const feed = useMemo(() => flattenRequestLogPages(query.data?.pages ?? []), [query.data]);

  const refreshLatest = useCallback(async () => {
    if (refreshingRef.current) {
      return;
    }
    refreshingRef.current = true;
    resetOnNextSyncRef.current = false;
    setIsRefreshing(true);
    generationRef.current += 1;
    const generation = generationRef.current;
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    try {
      await queryClient.cancelQueries({ queryKey, exact: true });
      const latest = await getRequestLogs(null, filters);
      if (scopeRef.current !== scope || generationRef.current !== generation) {
        return;
      }
      queryClient.setQueryData<InfiniteData<RequestLogList, string | null>>(queryKey, {
        pages: [latest],
        pageParams: [null],
      });
      setPending(null);
      setSyncFailure(null);
    } catch (error) {
      if (scopeRef.current === scope && generationRef.current === generation) {
        setSyncFailure({ scope, error: asError(error) });
      }
      throw error;
    } finally {
      refreshingRef.current = false;
      setIsRefreshing(false);
      if (rerunRef.current || eventCountRef.current > 0) {
        rerunRef.current = false;
        queueSyncRun();
      }
    }
  }, [filters, queryClient, queryKey, queueSyncRun, scope]);

  const applyPending = useCallback(() => {
    if (pending?.scope === scope) {
      followingRef.current = true;
      resetOnNextSyncRef.current = true;
      scheduleSync();
    }
  }, [pending, scheduleSync, scope]);

  const scopedFailure = syncFailure?.scope === scope ? syncFailure.error : null;

  return {
    ...query,
    ...feed,
    isFetching: query.isFetching || isRefreshing,
    isError: query.isError || scopedFailure !== null,
    error: query.error ?? scopedFailure,
    pendingCount: pending?.scope === scope ? pending.count : 0,
    refreshLatest,
    applyPending,
  };
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

function asError(value: unknown) {
  return value instanceof Error ? value : new Error("请求日志同步失败");
}
