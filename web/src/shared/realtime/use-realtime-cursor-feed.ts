import {
  useInfiniteQuery,
  useQueryClient,
  type InfiniteData,
  type QueryKey,
} from "@tanstack/react-query";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { collectCursorBatches } from "@/shared/lib/collect-cursor-batches";

const EVENT_BATCH_MS = 100;

interface CursorPage<TItem> {
  items: TItem[];
  nextCursor: string | null;
}

interface UseRealtimeCursorFeedOptions<
  TItem,
  TPage extends CursorPage<TItem>,
  TFeed extends object,
> {
  queryKey: QueryKey;
  scope: string;
  followingLatest: boolean;
  fetchPage: (cursor: string | null, signal?: AbortSignal) => Promise<TPage>;
  knownIds: (data: InfiniteData<TPage, string | null>) => ReadonlySet<string>;
  itemId: (item: TItem) => string;
  mergeLatest: (
    current: InfiniteData<TPage, string | null> | undefined,
    latest: InfiniteData<TPage, string | null>,
  ) => InfiniteData<TPage, string | null>;
  countNew: (
    current: InfiniteData<TPage, string | null>,
    latest: InfiniteData<TPage, string | null>,
  ) => number;
  flatten: (pages: readonly TPage[]) => TFeed;
  maxCachedPages: number;
  maxCollectedItems: number;
  currentExternalGeneration?: () => number;
  syncErrorMessage: string;
}

interface ScopedCount {
  scope: string;
  count: number;
}

interface ScopedFailure {
  scope: string;
  error: Error;
}

export function useRealtimeCursorFeed<
  TItem,
  TPage extends CursorPage<TItem>,
  TFeed extends object,
>({
  queryKey,
  scope,
  followingLatest,
  fetchPage,
  knownIds,
  itemId,
  mergeLatest,
  countNew,
  flatten,
  maxCachedPages,
  maxCollectedItems,
  currentExternalGeneration,
  syncErrorMessage,
}: UseRealtimeCursorFeedOptions<TItem, TPage, TFeed>) {
  const queryClient = useQueryClient();
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
  const [pending, setPending] = useState<ScopedCount | null>(null);
  const [syncFailure, setSyncFailure] = useState<ScopedFailure | null>(null);
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
    queryFn: ({ pageParam, signal }) => fetchPage(pageParam, signal),
    initialPageParam: null as string | null,
    getNextPageParam: (lastPage) => lastPage.nextCursor ?? undefined,
    maxPages: maxCachedPages,
    staleTime: Number.POSITIVE_INFINITY,
  });

  const operationIsCurrent = useCallback(
    (generation: number, externalGeneration: number | undefined) =>
      scopeRef.current === scope
      && generationRef.current === generation
      && currentExternalGeneration?.() === externalGeneration,
    [currentExternalGeneration, scope],
  );

  const applyLatest = useCallback(
    (latest: InfiniteData<TPage, string | null>) => {
      queryClient.setQueryData<InfiniteData<TPage, string | null>>(
        queryKey,
        (current) => mergeLatest(current, latest),
      );
      setPending(null);
    },
    [mergeLatest, queryClient, queryKey],
  );

  const syncLatest = useCallback(async () => {
    const generation = generationRef.current;
    const externalGeneration = currentExternalGeneration?.();
    const current = queryClient.getQueryData<InfiniteData<TPage, string | null>>(
      queryKey,
    );
    if (!current) {
      return;
    }
    const latest = await collectCursorBatches<TItem, TPage>(
      (cursor) => fetchPage(cursor),
      knownIds(current),
      itemId,
      maxCachedPages,
      maxCollectedItems,
    );
    if (!operationIsCurrent(generation, externalGeneration)) {
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
    setPending({ scope, count: countNew(current, latest) });
  }, [
    applyLatest,
    countNew,
    currentExternalGeneration,
    fetchPage,
    itemId,
    knownIds,
    maxCachedPages,
    maxCollectedItems,
    operationIsCurrent,
    queryClient,
    queryKey,
    scope,
  ]);

  const queueSyncRun = useCallback(() => {
    if (timerRef.current !== null) {
      return;
    }
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
    const externalGeneration = currentExternalGeneration?.();
    eventCountRef.current = 0;
    try {
      await syncLatest();
      if (operationIsCurrent(generation, externalGeneration)) {
        setSyncFailure(null);
      }
    } catch (error) {
      if (operationIsCurrent(generation, externalGeneration)) {
        setSyncFailure({ scope, error: asError(error, syncErrorMessage) });
      }
    } finally {
      syncingRef.current = false;
      if (rerunRef.current || eventCountRef.current > 0) {
        rerunRef.current = false;
        queueSyncRun();
      }
    }
  }, [
    currentExternalGeneration,
    operationIsCurrent,
    queueSyncRun,
    scope,
    syncErrorMessage,
    syncLatest,
  ]);

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

  const feed = useMemo(() => flatten(query.data?.pages ?? []), [flatten, query.data]);

  const refreshLatest = useCallback(async () => {
    if (refreshingRef.current) {
      return;
    }
    refreshingRef.current = true;
    resetOnNextSyncRef.current = false;
    setIsRefreshing(true);
    generationRef.current += 1;
    const generation = generationRef.current;
    const externalGeneration = currentExternalGeneration?.();
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    try {
      await queryClient.cancelQueries({ queryKey, exact: true });
      const latest = await fetchPage(null);
      if (!operationIsCurrent(generation, externalGeneration)) {
        return;
      }
      queryClient.setQueryData<InfiniteData<TPage, string | null>>(queryKey, {
        pages: [latest],
        pageParams: [null],
      });
      setPending(null);
      setSyncFailure(null);
    } catch (error) {
      if (operationIsCurrent(generation, externalGeneration)) {
        setSyncFailure({ scope, error: asError(error, syncErrorMessage) });
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
  }, [
    currentExternalGeneration,
    fetchPage,
    operationIsCurrent,
    queryClient,
    queryKey,
    queueSyncRun,
    scope,
    syncErrorMessage,
  ]);

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
    refreshLatest,
    applyPending,
    scheduleSync,
  };
}

function asError(value: unknown, message: string) {
  return value instanceof Error ? value : new Error(message);
}
