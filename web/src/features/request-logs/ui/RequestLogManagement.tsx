import { ArrowUp, RefreshCw, ScrollText } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import type { RequestLogFilters } from "../api/request-log-filter-contracts";
import { hasActiveRequestLogFilters } from "../api/request-log-filter-contracts";
import { isActiveRequestLog } from "../model/request-log-feed";
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
import { Surface } from "@/shared/ui/Surface";

export function RequestLogManagement() {
  const [filters, setFilters] = useState<RequestLogFilters>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [followingLatest, setFollowingLatest] = useState(true);
  const mobileTopRef = useRef<HTMLDivElement>(null);
  const query = useRequestLogs(filters, followingLatest);
  const realtime = useAdminRealtimeStatus();
  const nowMs = useActiveClock(query.activeTotal > 0);
  const { fetchNextPage, hasNextPage, isFetchingNextPage, refreshLatest } = query;

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
    if (typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches) {
      setFollowingLatest(visible);
    }
  }, []);

  const followPending = () => {
    setFollowingLatest(true);
    query.applyPending();
    mobileTopRef.current?.scrollIntoView?.({ block: "start" });
  };

  const changeFilters = (next: RequestLogFilters) => {
    setSelectedId(null);
    setFollowingLatest(true);
    mobileTopRef.current?.scrollIntoView?.({ block: "start" });
    setFilters(next);
  };

  if (query.isPending && !query.data) {
    return <Surface className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary" aria-busy="true">正在读取请求日志</Surface>;
  }

  if (!query.data || !query.telemetry || !query.filterOptions) {
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
        <p className="mr-auto text-[12px] text-secondary">
          队列 <Count>{query.telemetry.queuedRecords}</Count>
          <Divider />写入中 <Count>{query.telemetry.inFlightRecords}</Count>
          <Divider />丢弃 <Count>{query.telemetry.droppedRecords}</Count>
          {query.activeTotal > 0 ? <><Divider />进行中 <Count accent>{query.activeTotal}</Count></> : null}
          {!realtime.connected ? <><Divider /><span className="text-warning">实时连接中断</span></> : null}
        </p>
        <RequestLogFilterBar filters={filters} options={query.filterOptions} onChange={changeFilters} />
        <Button variant="ghost" onClick={() => void refreshLogs()} disabled={query.isFetching && !query.isFetchingNextPage}>
          <RefreshCw size={14} className={query.isFetching && !query.isFetchingNextPage ? "animate-spin" : undefined} />刷新
        </Button>
      </div>

      {query.isError ? (
        <Surface className="mt-3 shrink-0 border-warning/40 p-4 text-sm text-secondary" role="status">
          同步失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      {query.pendingCount > 0 ? (
        <button type="button" className="focus-ring mx-auto mt-3 inline-flex shrink-0 items-center gap-1.5 rounded-full bg-accent px-3 py-1.5 text-[12px] font-medium text-white shadow-sm" onClick={followPending}>
          <ArrowUp size={13} />有 {query.pendingCount} 条新日志 · 回到最新
        </button>
      ) : null}

      <div className="pt-3 md:min-h-0 md:flex-1">
        {query.items.length === 0 ? (
          <EmptyState filtered={hasActiveRequestLogFilters(filters)} />
        ) : (
          <>
            <div
              ref={mobileTopRef}
              className="management-scroll-viewport space-y-2 md:hidden"
              role="list"
              aria-label="请求日志列表"
            >
              <IntersectionSentinel onVisibilityChange={handleMobileLatest} />
              {query.items.map((item) =>
                isActiveRequestLog(item) ? (
                  <ActiveRequestLogCard key={item.requestId} log={item} nowMs={nowMs} />
                ) : (
                  <RequestLogCard key={item.requestId} log={item} selected={selectedId === item.requestId} onSelect={setSelectedId} />
                ),
              )}
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
            />
          </>
        )}
      </div>

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

function Count({ children, accent = false }: { children: number; accent?: boolean }) {
  return <span className={`tabular-nums ${accent ? "text-accent-copy" : "text-primary"}`}>{children}</span>;
}

function Divider() {
  return <span className="mx-1.5 text-tertiary">·</span>;
}
