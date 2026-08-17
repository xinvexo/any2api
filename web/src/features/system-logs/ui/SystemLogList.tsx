import { useCallback, useEffect, useRef } from "react";

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

interface SystemLogListProps {
  items: readonly SystemLog[];
  selectedId: string | null;
  followingLatest: boolean;
  hasMore: boolean;
  loadingMore: boolean;
  onSelect: (requestId: string) => void;
  onFollowingLatestChange: (following: boolean) => void;
  onLoadMore: () => void;
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
}: SystemLogListProps) {
  const mobileTopRef = useRef<HTMLDivElement>(null);
  const previousFollowingRef = useRef(followingLatest);
  const handleMobileLatest = useCallback(
    (visible: boolean) => {
      if (typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches) {
        onFollowingLatestChange(visible);
      }
    },
    [onFollowingLatestChange],
  );
  const handleHistoryVisible = useCallback(
    (visible: boolean) => { if (visible) onLoadMore(); },
    [onLoadMore],
  );

  useEffect(() => {
    if (followingLatest && !previousFollowingRef.current) {
      mobileTopRef.current?.scrollIntoView?.({ block: "start" });
    }
    previousFollowingRef.current = followingLatest;
  }, [followingLatest]);

  return (
    <>
      <div
        ref={mobileTopRef}
        className="management-scroll-viewport space-y-2 md:hidden"
        role="list"
        aria-label="系统日志列表"
      >
        <IntersectionSentinel onVisibilityChange={handleMobileLatest} />
        {items.map((log) => (
          <SystemLogCard key={log.requestId} log={log} selected={selectedId === log.requestId} onSelect={onSelect} />
        ))}
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
      />
    </>
  );
}

function SystemLogCard({ log, selected, onSelect }: { log: SystemLog; selected: boolean; onSelect: (requestId: string) => void }) {
  return (
    <div role="listitem">
      <button
        type="button"
        aria-pressed={selected}
        aria-label={`查看完整请求 ${log.uri}`}
        className={cn(
          "focus-ring block min-h-[4.5rem] w-full min-w-0 rounded-[8px] bg-surface-muted/45 px-3 py-2.5 text-left transition-colors",
          selected ? "bg-accent/10 ring-1 ring-accent/35" : "hover:bg-surface-muted/70",
        )}
        onClick={() => onSelect(log.requestId)}
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
      </button>
    </div>
  );
}
