import {
  observeElementRect as observeVirtualElementRect,
  useVirtualizer,
} from "@tanstack/react-virtual";
import { useLayoutEffect, useRef, type ReactNode } from "react";

import type { SystemLog } from "../api/system-log-contracts";
import {
  formatBytes,
  formatDuration,
  formatSystemLogTime,
  outcomeLabel,
  statusTone,
} from "../model/system-log-presentation";
import { cn } from "@/shared/lib/cn";
import {
  listEntrySurfaceAnimationClass,
  type ListEntryAnimation,
} from "@/shared/ui/useListEntryAnimations";

const ROW_HEIGHT = 44;
const gridClass =
  "grid grid-cols-[7rem_9rem_3.5rem_minmax(13rem,1fr)_3rem_4.5rem_4rem_4.5rem_4rem] items-center gap-2 px-2";

interface SystemLogVirtualTableProps {
  items: readonly SystemLog[];
  selectedId: string | null;
  followingLatest: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  onSelect: (requestId: string) => void;
  onFollowingLatestChange: (following: boolean) => void;
  onLoadMore: () => void;
  entryAnimations?: ReadonlyMap<string, ListEntryAnimation>;
}

export function SystemLogVirtualTable({
  items,
  selectedId,
  followingLatest,
  hasMore,
  loadingMore,
  onSelect,
  onFollowingLatestChange,
  onLoadMore,
  entryAnimations,
}: SystemLogVirtualTableProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const anchorRef = useRef({ ids: [] as string[], scrollTop: 0 });
  const rowCount = items.length + (hasMore ? 1 : 0);
  // TanStack Virtual exposes a mutable controller that React Compiler must not memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: rowCount,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => ROW_HEIGHT,
    getItemKey: (index) => items[index]?.requestId ?? "system-log-history-loader",
    overscan: 10,
    useFlushSync: false,
    initialRect: { width: 980, height: 640 },
    observeElementRect: (instance, callback) =>
      observeVirtualElementRect(instance, (rect) =>
        callback({ width: rect.width, height: rect.height > 0 ? rect.height : 640 }),
      ),
  });

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const previous = anchorRef.current;
    const nextIds = items.map((item) => item.requestId);
    if (followingLatest) {
      viewport.scrollTop = 0;
    } else if (previous.ids.length > 0 && nextIds.length > 0) {
      const oldFirstInNext = nextIds.indexOf(previous.ids[0]);
      const nextFirstInOld = previous.ids.indexOf(nextIds[0]);
      if (previous.scrollTop <= ROW_HEIGHT) {
        viewport.scrollTop = 0;
      } else if (oldFirstInNext > 0) {
        viewport.scrollTop = previous.scrollTop + oldFirstInNext * ROW_HEIGHT;
      } else if (nextFirstInOld > 0) {
        viewport.scrollTop = Math.max(0, previous.scrollTop - nextFirstInOld * ROW_HEIGHT);
      }
    }
    anchorRef.current = { ids: nextIds, scrollTop: viewport.scrollTop };
  }, [followingLatest, items]);

  const handleScroll = () => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    anchorRef.current.scrollTop = viewport.scrollTop;
    onFollowingLatestChange(viewport.scrollTop <= ROW_HEIGHT);
    const remaining = virtualizer.getTotalSize() - viewport.scrollTop - viewport.clientHeight;
    if (hasMore && !loadingMore && remaining <= ROW_HEIGHT * 10) onLoadMore();
  };

  return (
    <div className="hidden h-full min-h-0 overflow-x-auto md:block">
      <div role="table" aria-label="系统日志表格" aria-rowcount={items.length + 1} className="flex h-full min-w-[920px] flex-col">
        <div role="rowgroup" aria-label="系统日志表头" className="shrink-0 overflow-y-scroll border-b border-subtle [scrollbar-gutter:stable]">
          <div role="row" aria-rowindex={1} className={cn(gridClass, "text-[11px] font-medium text-tertiary")}>
            <Header>时间</Header><Header>客户端</Header><Header>方法</Header><Header>请求 URI</Header><Header>状态</Header><Header>协议</Header><Header>耗时</Header><Header>响应</Header><Header>结果</Header>
          </div>
        </div>
        <div
          ref={viewportRef}
          role="rowgroup"
          aria-label="系统日志表格数据"
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
          tabIndex={0}
          className="focus-ring min-h-0 flex-1 overflow-y-scroll outline-none [scrollbar-gutter:stable]"
          onScroll={handleScroll}
        >
          <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const log = items[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  className="absolute left-0 top-0 w-full"
                  style={{ height: ROW_HEIGHT, transform: `translateY(${virtualRow.start}px)` }}
                >
                  {log ? (
                    <SystemLogRow
                      log={log}
                      selected={selectedId === log.requestId}
                      animation={entryAnimations?.get(log.requestId)}
                      onSelect={onSelect}
                    />
                  ) : (
                    <div role="row" className="grid h-11 place-items-center text-[11px] text-tertiary">{loadingMore ? "正在加载更早记录" : "继续滚动加载更早记录"}</div>
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

function SystemLogRow({
  log,
  selected,
  animation,
  onSelect,
}: {
  log: SystemLog;
  selected: boolean;
  animation?: ListEntryAnimation;
  onSelect: (requestId: string) => void;
}) {
  return (
    <div
      role="row"
      tabIndex={0}
      aria-selected={selected}
      title="双击查看详情"
      className={cn(
        gridClass,
        "compact-row-surface compact-row-surface-hover focus-ring h-11 cursor-pointer rounded-[8px] text-[12px] outline-none",
        selected && "compact-row-surface-selected",
        listEntrySurfaceAnimationClass(animation),
      )}
      onDoubleClick={() => onSelect(log.requestId)}
      onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); onSelect(log.requestId); } }}
    >
      <Cell className="tabular-nums text-secondary">{formatSystemLogTime(log.startedAtMs)}</Cell>
      <Cell className="font-mono text-secondary" title={log.clientIp ?? "未知"}>{log.clientIp ?? "未知"}</Cell>
      <Cell className="font-mono font-semibold">{log.method}</Cell>
      <Cell className="font-mono" title={log.uri}>{log.uri}</Cell>
      <Cell className={cn("font-mono font-semibold", statusTone(log))}>{log.statusCode ?? "-"}</Cell>
      <Cell className="font-mono text-secondary">{log.httpVersion}</Cell>
      <Cell className="tabular-nums text-secondary">{formatDuration(log.durationMs)}</Cell>
      <Cell className="tabular-nums text-secondary">{formatBytes(log.responseBytes)}</Cell>
      <Cell className="text-secondary">{outcomeLabel(log.outcome)}</Cell>
    </div>
  );
}

function Header({ children }: { children: ReactNode }) {
  return <div role="columnheader" className="min-w-0 px-1 py-2 text-left">{children}</div>;
}

function Cell({ children, className, title }: { children: ReactNode; className?: string; title?: string }) {
  return <div role="cell" title={title} className={cn("min-w-0 truncate px-1", className)}>{children}</div>;
}
