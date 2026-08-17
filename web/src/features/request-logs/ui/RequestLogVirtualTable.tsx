import {
  observeElementRect as observeVirtualElementRect,
  useVirtualizer,
} from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, type ReactNode } from "react";

import {
  isActiveRequestLog,
  type RequestLogFeedItem,
} from "../model/request-log-feed";
import { ActiveRequestLogTableRow } from "./ActiveRequestLogRow";
import {
  REQUEST_LOG_ROW_HEIGHT,
  RequestLogTableRow,
  requestLogGridClass,
} from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";
import type { ListEntryAnimation } from "@/shared/ui/useListEntryAnimations";

interface RequestLogVirtualTableProps {
  items: readonly RequestLogFeedItem[];
  selectedId: string | null;
  nowMs: number;
  followingLatest: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  onSelect: (requestId: string) => void;
  onFollowingLatestChange: (following: boolean) => void;
  onLoadMore: () => void;
  entryAnimations?: ReadonlyMap<string, ListEntryAnimation>;
}

export function RequestLogVirtualTable({
  items,
  selectedId,
  nowMs,
  followingLatest,
  hasMore,
  loadingMore,
  onSelect,
  onFollowingLatestChange,
  onLoadMore,
  entryAnimations,
}: RequestLogVirtualTableProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const anchorRef = useRef({ ids: [] as string[], scrollTop: 0 });
  const rowCount = items.length + (hasMore ? 1 : 0);
  // TanStack Virtual exposes a mutable controller that React Compiler must not memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: rowCount,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => REQUEST_LOG_ROW_HEIGHT,
    getItemKey: (index) => items[index]?.requestId ?? "request-log-history-loader",
    overscan: 10,
    useFlushSync: false,
    initialRect: { width: 1_120, height: 640 },
    observeElementRect: (instance, callback) =>
      observeVirtualElementRect(instance, (rect) =>
        callback({ width: rect.width, height: rect.height > 0 ? rect.height : 640 }),
      ),
  });

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    const previous = anchorRef.current;
    const nextIds = items.map((item) => item.requestId);
    if (followingLatest) {
      viewport.scrollTop = 0;
    } else if (previous.ids.length > 0 && nextIds.length > 0) {
      const oldFirstInNext = nextIds.indexOf(previous.ids[0]);
      const nextFirstInOld = previous.ids.indexOf(nextIds[0]);
      if (previous.scrollTop <= REQUEST_LOG_ROW_HEIGHT) {
        viewport.scrollTop = 0;
      } else if (oldFirstInNext > 0) {
        viewport.scrollTop = previous.scrollTop + oldFirstInNext * REQUEST_LOG_ROW_HEIGHT;
      } else if (nextFirstInOld > 0) {
        viewport.scrollTop = Math.max(
          0,
          previous.scrollTop - nextFirstInOld * REQUEST_LOG_ROW_HEIGHT,
        );
      }
    }
    anchorRef.current = { ids: nextIds, scrollTop: viewport.scrollTop };
  }, [followingLatest, items]);

  const handleScroll = () => {
    const viewport = viewportRef.current;
    if (!viewport) {
      return;
    }
    anchorRef.current.scrollTop = viewport.scrollTop;
    onFollowingLatestChange(viewport.scrollTop <= REQUEST_LOG_ROW_HEIGHT);
    const remaining = virtualizer.getTotalSize() - viewport.scrollTop - viewport.clientHeight;
    if (hasMore && !loadingMore && remaining <= REQUEST_LOG_ROW_HEIGHT * 10) {
      onLoadMore();
    }
  };

  return (
    <div className="hidden h-full min-h-0 overflow-x-auto md:block [scrollbar-gutter:stable]">
      <div role="table" aria-label="请求日志表格" aria-rowcount={items.length + 1} className="flex h-full min-w-[76rem] flex-col">
        <div role="rowgroup" aria-label="请求日志表头" className="shrink-0 overflow-y-scroll border-b border-subtle [scrollbar-gutter:stable]">
          <div role="row" aria-rowindex={1} className={cn(requestLogGridClass, "text-[11px] font-medium text-tertiary")}>
            <Header>时间</Header>
            <Header>客户端 IP</Header>
            <Header>令牌</Header>
            <Header>模型</Header>
            <Header>思考</Header>
            <Header>结果</Header>
            <Header>首 Token</Header>
            <Header>总耗时</Header>
            <Header>输入 Token</Header>
            <Header>输出 Token</Header>
            <Header>缓存命中 Token</Header>
            <Header>TPS</Header>
          </div>
        </div>
        <div
          ref={viewportRef}
          role="rowgroup"
          aria-label="请求日志表格数据"
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
          tabIndex={0}
          className="focus-ring min-h-0 flex-1 overflow-y-scroll outline-none [scrollbar-gutter:stable]"
          onScroll={handleScroll}
        >
          <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const item = items[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  className="absolute left-0 top-0 w-full"
                  style={{ height: REQUEST_LOG_ROW_HEIGHT, transform: `translateY(${virtualRow.start}px)` }}
                >
                  {item ? (
                    isActiveRequestLog(item) ? (
                      <ActiveRequestLogTableRow
                        log={item}
                        nowMs={nowMs}
                        animation={entryAnimations?.get(item.requestId)}
                      />
                    ) : (
                      <RequestLogTableRow
                        log={item}
                        selected={selectedId === item.requestId}
                        onSelect={onSelect}
                        animation={entryAnimations?.get(item.requestId)}
                      />
                    )
                  ) : (
                    <div role="row" className="grid h-11 place-items-center text-[11px] text-tertiary">
                      {loadingMore ? "正在加载更早记录" : "继续滚动加载更早记录"}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

function Header({ children }: { children: ReactNode }) {
  return <div role="columnheader" className="min-w-0 whitespace-nowrap px-1 py-2 text-left">{children}</div>;
}
