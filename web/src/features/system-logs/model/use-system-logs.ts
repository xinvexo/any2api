import {
  useInfiniteQuery,
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

import { getSystemLogs } from "../api/system-log-api";
import type { SystemLog, SystemLogList } from "../api/system-log-contracts";
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
import { collectCursorBatches } from "@/shared/lib/collect-cursor-batches";
import { useAdminEvent } from "@/shared/realtime";

const EVENT_BATCH_MS = 100;

interface PendingLatest {
  scope: boolean;
  count: number;
}

type SyncFailure = { scope: boolean; error: Error };

export function useSystemLogs(
  showAdminOperations: boolean,
  followingLatest: boolean,
) {
  const queryClient = useQueryClient();
  const queryKey = systemLogQueryKeys.list(showAdminOperations);
  const followingRef = useRef(followingLatest);
  const scopeRef = useRef(showAdminOperations);
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
    if (scopeRef.current !== showAdminOperations) {
      scopeRef.current = showAdminOperations;
      generationRef.current += 1;
      resetOnNextSyncRef.current = false;
      queryClient.removeQueries({ queryKey, exact: true });
      setPending(null);
      setSyncFailure(null);
    }
  }, [queryClient, queryKey, showAdminOperations]);

  const query = useInfiniteQuery({
    queryKey,
    queryFn: ({ pageParam, signal }) =>
      getSystemLogs(showAdminOperations, pageParam, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    maxPages: MAX_CACHED_SYSTEM_BATCHES,
    staleTime: Number.POSITIVE_INFINITY,
  });

  const applyLatest = useCallback(
    (latest: InfiniteData<SystemLogList, string | null>) => {
      queryClient.setQueryData<InfiniteData<SystemLogList, string | null>>(
        queryKey,
        (current) => mergeLatestSystemBatches(current, latest),
      );
      setPending(null);
    },
    [queryClient, queryKey],
  );

  const syncLatest = useCallback(async () => {
    const generation = generationRef.current;
    const feedGeneration = currentSystemLogFeedGeneration();
    const current = queryClient.getQueryData<InfiniteData<SystemLogList, string | null>>(
      queryKey,
    );
    if (!current) {
      return;
    }
    const latest = await collectCursorBatches<SystemLog, SystemLogList>(
      (cursor) => getSystemLogs(showAdminOperations, cursor),
      systemLogIds(current),
      (item) => item.requestId,
      MAX_CACHED_SYSTEM_BATCHES,
      MAX_CACHED_SYSTEM_LOGS,
    );
    if (
      scopeRef.current !== showAdminOperations ||
      generationRef.current !== generation ||
      currentSystemLogFeedGeneration() !== feedGeneration
    ) {
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
      scope: showAdminOperations,
      count: countNewSystemLogs(current, latest),
    });
  }, [applyLatest, queryClient, queryKey, showAdminOperations]);

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
    const feedGeneration = currentSystemLogFeedGeneration();
    eventCountRef.current = 0;
    try {
      await syncLatest();
      if (
        scopeRef.current === showAdminOperations &&
        generationRef.current === generation &&
        currentSystemLogFeedGeneration() === feedGeneration
      ) {
        setSyncFailure(null);
      }
    } catch (error) {
      if (
        scopeRef.current === showAdminOperations &&
        generationRef.current === generation &&
        currentSystemLogFeedGeneration() === feedGeneration
      ) {
        setSyncFailure({ scope: showAdminOperations, error: asError(error) });
      }
    } finally {
      syncingRef.current = false;
      if (rerunRef.current || eventCountRef.current > 0) {
        rerunRef.current = false;
        queueSyncRun();
      }
    }
  }, [queueSyncRun, showAdminOperations, syncLatest]);

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

  useAdminEvent("system_logs_changed", true, scheduleSync);

  useEffect(() => {
    scheduleSync();
  }, [scheduleSync, showAdminOperations]);

  useEffect(() => {
    followingRef.current = followingLatest;
    if (
      followingLatest &&
      pending?.scope === showAdminOperations &&
      pending.count > 0
    ) {
      resetOnNextSyncRef.current = true;
      scheduleSync();
    }
  }, [followingLatest, pending, scheduleSync, showAdminOperations]);

  useEffect(() => {
    return () => {
      if (timerRef.current !== null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      eventCountRef.current = 0;
      rerunRef.current = false;
    };
  }, [showAdminOperations]);

  const feed = useMemo(() => flattenSystemLogPages(query.data?.pages ?? []), [query.data]);

  const refreshLatest = useCallback(async () => {
    if (refreshingRef.current) {
      return;
    }
    refreshingRef.current = true;
    resetOnNextSyncRef.current = false;
    setIsRefreshing(true);
    generationRef.current += 1;
    const generation = generationRef.current;
    const feedGeneration = currentSystemLogFeedGeneration();
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    try {
      await queryClient.cancelQueries({ queryKey, exact: true });
      const latest = await getSystemLogs(showAdminOperations);
      if (
        scopeRef.current !== showAdminOperations ||
        generationRef.current !== generation ||
        currentSystemLogFeedGeneration() !== feedGeneration
      ) {
        return;
      }
      queryClient.setQueryData<InfiniteData<SystemLogList, string | null>>(queryKey, {
        pages: [latest],
        pageParams: [null],
      });
      setPending(null);
      setSyncFailure(null);
    } catch (error) {
      if (
        scopeRef.current === showAdminOperations &&
        generationRef.current === generation &&
        currentSystemLogFeedGeneration() === feedGeneration
      ) {
        setSyncFailure({ scope: showAdminOperations, error: asError(error) });
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
  }, [queryClient, queryKey, queueSyncRun, showAdminOperations]);

  const applyPending = useCallback(() => {
    if (pending?.scope === showAdminOperations) {
      followingRef.current = true;
      resetOnNextSyncRef.current = true;
      scheduleSync();
    }
  }, [pending, scheduleSync, showAdminOperations]);

  const scopedFailure =
    syncFailure?.scope === showAdminOperations ? syncFailure.error : null;

  return {
    ...query,
    ...feed,
    isFetching: query.isFetching || isRefreshing,
    isError: query.isError || scopedFailure !== null,
    error: query.error ?? scopedFailure,
    refreshLatest,
    applyPending,
  };
}

function asError(value: unknown) {
  return value instanceof Error ? value : new Error("系统日志同步失败");
}
