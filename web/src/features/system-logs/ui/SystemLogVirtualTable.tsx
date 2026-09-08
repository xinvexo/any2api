import { useMemo, type ReactNode } from "react";
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
import { cn } from "@/shared/lib/cn";
import { AnchoredVirtualRows } from "@/shared/ui/AnchoredVirtualRows";
import {
  listEntrySurfaceAnimationClass,
  type ListEntryAnimation,
} from "@/shared/ui/useListEntryAnimations";

const ROW_HEIGHT = 52;
const gridClass =
  "grid w-full grid-cols-[7rem_3.5rem_minmax(10rem,1fr)_7.5rem_3.5rem_4rem_2.5rem] items-center gap-x-2 px-2";

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
  const itemIds = useMemo(() => items.map((item) => item.requestId), [items]);

  return (
    <div className="h-full min-h-0 overflow-x-auto [scrollbar-gutter:stable]">
      <div role="table" aria-label="系统日志表格" aria-rowcount={items.length + 1} className="flex h-full min-w-[43rem] flex-col">
        <div role="rowgroup" aria-label="系统日志表头" className="shrink-0 overflow-y-scroll border-b border-subtle [scrollbar-gutter:stable]">
          <div role="row" aria-rowindex={1} className={cn(gridClass, "text-[12px] font-medium text-secondary")}>
            <Header>时间</Header><Header>状态</Header><Header>请求路径</Header><Header>客户端 IP</Header><Header>耗时</Header><Header>响应</Header><Header><span className="sr-only">详情</span></Header>
          </div>
        </div>
        <AnchoredVirtualRows
          itemIds={itemIds}
          rowHeight={ROW_HEIGHT}
          followingLatest={followingLatest}
          hasMore={hasMore}
          loadingMore={loadingMore}
          historyLoaderKey="system-log-history-loader"
          initialWidth={980}
          ariaLabel="系统日志表格数据"
          onFollowingLatestChange={onFollowingLatestChange}
          onLoadMore={onLoadMore}
          renderRow={(index) => {
            const log = items[index]!;
            return (
              <SystemLogRow
                log={log}
                ariaRowIndex={index + 2}
                selected={selectedId === log.requestId}
                animation={entryAnimations?.get(log.requestId)}
                onSelect={onSelect}
              />
            );
          }}
          renderHistoryLoader={(loading) => (
            <div role="row" className="grid h-11 place-items-center text-[11px] text-tertiary">
              {loading ? "正在加载更早记录" : "继续滚动加载更早记录"}
            </div>
          )}
        />
      </div>
    </div>
  );
}

function SystemLogRow({
  log,
  ariaRowIndex,
  selected,
  animation,
  onSelect,
}: {
  log: SystemLog;
  ariaRowIndex: number;
  selected: boolean;
  animation?: ListEntryAnimation;
  onSelect: (requestId: string) => void;
}) {
  return (
    <div
      role="row"
      aria-rowindex={ariaRowIndex}
      tabIndex={0}
      aria-selected={selected}
      title="双击查看详情"
      className={cn(
        gridClass,
        "compact-row-surface compact-row-surface-hover focus-ring h-[52px] cursor-pointer rounded-[8px] text-[12px] outline-none",
        selected && "compact-row-surface-selected",
        listEntrySurfaceAnimationClass(animation),
      )}
      onDoubleClick={() => onSelect(log.requestId)}
      onKeyDown={(event) => { if (event.target === event.currentTarget && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); onSelect(log.requestId); } }}
    >
      <Cell className="tabular-nums text-secondary">{formatSystemLogTime(log.startedAtMs)}</Cell>
      <Cell truncate={false} className={cn("flex flex-col gap-0.5", statusTone(log))} title={`${log.httpVersion} · ${outcomeLabel(log.outcome)}`}><span className="font-mono font-semibold">{log.statusCode ?? "—"}</span>{log.outcome !== "completed" ? <span className="text-[11px]">{outcomeLabel(log.outcome)}</span> : null}</Cell>
      <Cell title={`${log.method} ${log.path}`} truncate={false} className="flex min-w-0 items-center gap-2"><span className="shrink-0 rounded bg-surface-muted px-1.5 py-1 font-mono text-[11px] text-secondary">{log.method}</span><span className="truncate font-mono">{log.path}</span></Cell>
      <Cell className="font-mono text-secondary" title={log.clientIp ?? "未知"}>{log.clientIp ?? "未知"}</Cell>
      <Cell className="tabular-nums text-secondary">{formatDuration(log.durationMs)}</Cell>
      <Cell className="tabular-nums text-secondary">{formatBytes(log.responseBytes)}</Cell>
      <Cell truncate={false}><RowActionButton label={`查看 HTTP 请求详情 ${log.path}`} onClick={() => onSelect(log.requestId)}><ChevronRight size={14} /></RowActionButton></Cell>
    </div>
  );
}

function Header({ children }: { children: ReactNode }) {
  return <div role="columnheader" className="min-w-0 whitespace-nowrap px-1 py-2 text-left">{children}</div>;
}

function Cell({
  children,
  className,
  title,
  truncate = true,
}: {
  children: ReactNode;
  className?: string;
  title?: string;
  truncate?: boolean;
}) {
  return <div role="cell" title={title} className={cn("min-w-0 px-1", truncate && "truncate", className)}>{children}</div>;
}
