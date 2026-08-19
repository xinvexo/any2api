import { useCallback } from "react";

import type { SystemLog } from "../api/system-log-contracts";
import {
  formatBytes,
  formatDuration,
  formatSystemLogTime,
  outcomeLabel,
  statusTone,
} from "../model/system-log-presentation";
import { SystemLogVirtualTable } from "./SystemLogVirtualTable";
import { cn } from "@/shared/lib/cn";
import { IntersectionSentinel } from "@/shared/ui/IntersectionSentinel";
import {
  listEntryAnimationClass,
  type ListEntryAnimation,
} from "@/shared/ui/useListEntryAnimations";
import { WindowVirtualList } from "@/shared/ui/WindowVirtualList";

interface SystemLogListProps {
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

export function SystemLogList({
  items,
  selectedId,
  followingLatest,
  hasMore,
  loadingMore,
  onSelect,
  onFollowingLatestChange,
  onLoadMore,
  entryAnimations,
}: SystemLogListProps) {
  const handleMobileLatest = useCallback(
    (visible: boolean) => {
      if (isMobileViewport()) {
        onFollowingLatestChange(visible);
      }
    },
    [onFollowingLatestChange],
  );
  const handleHistoryVisible = useCallback(
    (visible: boolean) => { if (visible) onLoadMore(); },
    [onLoadMore],
  );
  return (
    <>
      <div
        className="management-scroll-viewport space-y-2 md:hidden"
      >
        <IntersectionSentinel onVisibilityChange={handleMobileLatest} />
        <WindowVirtualList
          items={items}
          getItemKey={(log) => log.requestId}
          renderItem={(log) => (
            <SystemLogCard log={log} selected={selectedId === log.requestId} onSelect={onSelect} />
          )}
          ariaLabel="系统日志列表"
          estimateItemHeight={72}
          getItemClassName={(log) => listEntryAnimationClass(entryAnimations?.get(log.requestId))}
        />
        <IntersectionSentinel enabled={hasMore && !loadingMore} rootMargin="400px 0px" onVisibilityChange={handleHistoryVisible} />
        {loadingMore ? <p className="py-3 text-center text-[12px] text-tertiary">正在加载更早记录</p> : null}
      </div>
      <SystemLogVirtualTable
        items={items}
        selectedId={selectedId}
        followingLatest={followingLatest}
        hasMore={hasMore}
        loadingMore={loadingMore}
        onSelect={onSelect}
        onFollowingLatestChange={onFollowingLatestChange}
        onLoadMore={onLoadMore}
        entryAnimations={entryAnimations}
      />
    </>
  );
}

function isMobileViewport() {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(max-width: 767px)").matches
  );
}

function SystemLogCard({ log, selected, onSelect }: { log: SystemLog; selected: boolean; onSelect: (requestId: string) => void }) {
  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        aria-pressed={selected}
        aria-label={`查看完整请求 ${log.uri}`}
        title="双击查看详情"
        className={cn(
          "focus-ring block min-h-[4.5rem] w-full min-w-0 cursor-pointer select-text rounded-[8px] bg-surface-muted/45 px-3 py-2.5 text-left outline-none transition-colors",
          selected ? "bg-accent/10 ring-1 ring-accent/35" : "hover:bg-surface-muted/70",
        )}
        onDoubleClick={() => onSelect(log.requestId)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            onSelect(log.requestId);
          }
        }}
      >
      <div className="flex min-w-0 items-center gap-2">
        <time className="shrink-0 text-[11px] tabular-nums text-tertiary" dateTime={new Date(log.startedAtMs).toISOString()}>
          {formatSystemLogTime(log.startedAtMs)}
        </time>
        <span className="shrink-0 rounded-[5px] bg-surface/70 px-1.5 py-0.5 font-mono text-[10px] font-semibold text-secondary">{log.method}</span>
        <span className="min-w-0 flex-1 truncate font-mono text-[12px] text-primary" title={log.uri}>{log.uri}</span>
        <span className={cn("shrink-0 font-mono text-[11px] font-semibold", statusTone(log))}>{log.statusCode ?? "-"}</span>
      </div>
      <div className="mt-1.5 flex min-w-0 items-center gap-2 text-[11px] text-secondary">
        <span className="min-w-0 flex-1 truncate font-mono">{log.clientIp ?? "未知"}</span>
        <span className="shrink-0">{formatDuration(log.durationMs)}</span>
        <span className="shrink-0">{formatBytes(log.responseBytes)}</span>
        <span className="shrink-0">{outcomeLabel(log.outcome)}</span>
      </div>
      </div>
    </div>
  );
}
