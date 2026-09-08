import { useCallback } from "react";
import { ChevronRight } from "lucide-react";
import { RowActionButton } from "@/shared/ui/RowActionButton";

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
import { useMobileViewport } from "@/shared/ui/use-mobile-viewport";

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
  const mobile = useMobileViewport();
  const handleHistoryVisible = useCallback(
    (visible: boolean) => { if (visible) onLoadMore(); },
    [onLoadMore],
  );
  return mobile ? (
    <div className="management-scroll-viewport space-y-2">
      <IntersectionSentinel onVisibilityChange={onFollowingLatestChange} />
      <WindowVirtualList
        items={items}
        getItemKey={(log) => log.requestId}
        renderItem={(log) => (
          <SystemLogCard log={log} selected={selectedId === log.requestId} onSelect={onSelect} />
        )}
        ariaLabel="系统日志列表"
        estimateItemHeight={136}
        getItemClassName={(log) => listEntryAnimationClass(entryAnimations?.get(log.requestId))}
      />
      <IntersectionSentinel enabled={hasMore && !loadingMore} rootMargin="400px 0px" onVisibilityChange={handleHistoryVisible} />
      {loadingMore ? <p className="py-3 text-center text-[12px] text-tertiary">正在加载更早记录</p> : null}
    </div>
  ) : (
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
  );
}

function SystemLogCard({ log, selected, onSelect }: { log: SystemLog; selected: boolean; onSelect: (requestId: string) => void }) {
  return <article className={cn("my-1 min-w-0 rounded-xl border border-subtle bg-surface p-3", selected && "border-accent/35 bg-accent/5")}>
    <div className="flex min-w-0 items-center justify-between gap-2 text-[12px]">
      <time className="tabular-nums text-secondary" dateTime={new Date(log.startedAtMs).toISOString()}>{formatSystemLogTime(log.startedAtMs)}</time>
      <span className={cn("font-mono font-semibold", statusTone(log))}>{log.statusCode ?? "—"} · {outcomeLabel(log.outcome)}</span>
    </div>
    <div className="mt-2 flex min-w-0 items-start gap-2">
      <span className="shrink-0 rounded bg-surface-muted px-1.5 py-1 font-mono text-[11px] font-semibold text-secondary">{log.method}</span>
      <p className="min-w-0 break-all font-mono text-[12px] leading-5">{log.path}</p>
    </div>
    <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[12px] text-secondary">
      <span className="font-mono">{log.clientIp ?? "未知 IP"}</span>
      <span>{formatDuration(log.durationMs)}</span>
      <span>{formatBytes(log.responseBytes)}</span>
      <RowActionButton className="ml-auto" label={`查看 HTTP 请求详情 ${log.path}`} onClick={() => onSelect(log.requestId)}>详情<ChevronRight size={14} /></RowActionButton>
    </div>
  </article>;
}
