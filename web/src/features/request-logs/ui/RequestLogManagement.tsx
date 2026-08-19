import { RefreshCw, ScrollText } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { RequestLogFilters } from "../api/request-log-filter-contracts";
import { hasActiveRequestLogFilters } from "../api/request-log-filter-contracts";
import {
  isActiveRequestLog,
  type RequestLogFeedItem,
} from "../model/request-log-feed";
import { getRequestLogErrorMessage } from "../model/request-log-error";
import { useRequestLogs } from "../model/use-request-logs";
import { ActiveRequestLogCard } from "./ActiveRequestLogRow";
import { RequestLogDetailDrawer } from "./RequestLogDetailDrawer";
import { RequestLogFilterBar } from "./RequestLogFilterBar";
import { RequestLogCard } from "./RequestLogTableRow";
import { RequestLogVirtualTable } from "./RequestLogVirtualTable";
import { notify } from "@/shared/notifications";
import { useAdminRealtimeStatus } from "@/shared/realtime";
import { Button } from "@/shared/ui/Button";
import { IntersectionSentinel } from "@/shared/ui/IntersectionSentinel";
import { ScrollToTopButton } from "@/shared/ui/ScrollToTopButton";
import { Surface } from "@/shared/ui/Surface";
import {
  listEntryAnimationClass,
  useListEntryAnimations,
} from "@/shared/ui/useListEntryAnimations";
import { WindowVirtualList } from "@/shared/ui/WindowVirtualList";

export function RequestLogManagement() {
  const [filters, setFilters] = useState<RequestLogFilters>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [followingLatest, setFollowingLatest] = useState(true);
  const mobileTopRef = useRef<HTMLDivElement>(null);
  const query = useRequestLogs(filters, followingLatest);
  const realtime = useAdminRealtimeStatus();
  const nowMs = useActiveClock(query.activeTotal > 0);
  const entryAnimations = useListEntryAnimations(
    query.items,
    requestEntryId,
    requestEntryState,
    `${filters.outcome ?? ""}\u0000${filters.publicModel ?? ""}\u0000${filters.gatewayApiKeyId ?? ""}\u0000${query.data ? "ready" : "loading"}`,
  );
  const {
    applyPending,
    fetchNextPage,
    hasNextPage,
    isFetchingNextPage,
    refreshLatest,
  } = query;

  const refreshLogs = useCallback(async () => {
    setFollowingLatest(true);
    try {
      await refreshLatest();
      mobileTopRef.current?.scrollIntoView?.({ block: "start" });
      notify.success("请求日志已刷新");
    } catch {
      notify.danger("请求日志刷新失败");
    }
  }, [refreshLatest]);

  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) {
      void fetchNextPage();
    }
  }, [fetchNextPage, hasNextPage, isFetchingNextPage]);

  const handleMobileLatest = useCallback((visible: boolean) => {
    if (isMobileViewport()) {
      setFollowingLatest(visible);
    }
  }, []);

  const changeFilters = (next: RequestLogFilters) => {
    setSelectedId(null);
    setFollowingLatest(true);
    mobileTopRef.current?.scrollIntoView?.({ block: "start" });
    setFilters(next);
  };

  const scrollToTop = useCallback(() => {
    setFollowingLatest(true);
    applyPending();
  }, [applyPending]);

  if (query.isPending && !query.data) {
    return <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">正在读取请求日志</Surface>;
  }

  if (!query.data || !query.filterOptions) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取请求日志</p>
        <p className="mt-2 text-sm text-secondary">{getRequestLogErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void refreshLogs()} disabled={query.isFetching}>
          <RefreshCw size={15} />重试
        </Button>
      </Surface>
    );
  }

  return (
    <div className="flex flex-1 flex-col md:h-full md:min-h-0 md:overflow-hidden" aria-busy={query.isFetching}>
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-subtle pb-3">
        {query.activeTotal > 0 || !realtime.connected ? (
          <div className="flex w-full items-center gap-3 text-[12px] text-secondary sm:mr-auto sm:w-auto">
            {query.activeTotal > 0 ? (
              <span>进行中 <span className="tabular-nums text-accent-copy">{query.activeTotal}</span></span>
            ) : null}
            {!realtime.connected ? <span className="text-warning">实时连接中断</span> : null}
          </div>
        ) : null}
        <RequestLogFilterBar
          filters={filters}
          options={query.filterOptions}
          onChange={changeFilters}
          onRefresh={() => void refreshLogs()}
          refreshing={query.isFetching && !query.isFetchingNextPage}
        />
      </div>

      {query.isError ? (
        <Surface className="mt-3 shrink-0 border-warning/40 p-4 text-sm text-secondary" role="status">
          同步失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      <div className="pt-3 md:min-h-0 md:flex-1">
        {query.items.length === 0 ? (
          <EmptyState filtered={hasActiveRequestLogFilters(filters)} />
        ) : (
          <>
            <div
              ref={mobileTopRef}
              className="management-scroll-viewport space-y-2 md:hidden"
            >
              <IntersectionSentinel onVisibilityChange={handleMobileLatest} />
              <WindowVirtualList
                items={query.items}
                getItemKey={requestEntryId}
                renderItem={(item) =>
                  isActiveRequestLog(item) ? (
                    <ActiveRequestLogCard log={item} nowMs={nowMs} />
                  ) : (
                    <RequestLogCard log={item} selected={selectedId === item.requestId} onSelect={setSelectedId} />
                  )
                }
                ariaLabel="请求日志列表"
                estimateItemHeight={72}
                getItemClassName={(item) => listEntryAnimationClass(entryAnimations.get(item.requestId))}
              />
              <IntersectionSentinel
                enabled={query.hasNextPage && !query.isFetchingNextPage}
                rootMargin="400px 0px"
                onVisibilityChange={(visible) => { if (visible) loadMore(); }}
              />
              {query.isFetchingNextPage ? <p className="py-3 text-center text-[12px] text-tertiary">正在加载更早记录</p> : null}
            </div>
            <RequestLogVirtualTable
              items={query.items}
              selectedId={selectedId}
              nowMs={nowMs}
              followingLatest={followingLatest}
              hasMore={query.hasNextPage}
              loadingMore={query.isFetchingNextPage}
              onSelect={setSelectedId}
              onFollowingLatestChange={setFollowingLatest}
              onLoadMore={loadMore}
              entryAnimations={entryAnimations}
            />
          </>
        )}
      </div>

      <ScrollToTopButton visible={!followingLatest} onClick={scrollToTop} />

      <RequestLogDetailDrawer requestId={selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}

function useActiveClock(enabled: boolean) {
  const [nowMs, setNowMs] = useState(Date.now);
  useEffect(() => {
    if (!enabled) return;
    const timer = window.setInterval(() => setNowMs(Date.now()), 1_000);
    return () => window.clearInterval(timer);
  }, [enabled]);
  return nowMs;
}

function EmptyState({ filtered }: { filtered: boolean }) {
  return <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center"><ScrollText size={22} className="text-tertiary" /><p className="mt-3 text-[13px] font-medium">{filtered ? "没有匹配的请求日志" : "还没有请求日志"}</p><p className="mt-1 text-[12px] text-secondary">通过网关完成一次请求后，记录会出现在这里。</p></div>;
}

function requestEntryId(item: RequestLogFeedItem) {
  return item.requestId;
}

function requestEntryState(item: RequestLogFeedItem) {
  return isActiveRequestLog(item) ? "processing" : item.outcome;
}

function isMobileViewport() {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(max-width: 767px)").matches
  );
}
