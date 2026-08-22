import { useMemo, type ReactNode } from "react";

import {
  isActiveRequestLog,
  type RequestLogFeedItem,
} from "../model/request-log-feed";
import { ActiveRequestLogTableCells } from "./ActiveRequestLogRow";
import { RequestAttemptMarker } from "./RequestAttemptMarker";
import {
  REQUEST_LOG_ROW_HEIGHT,
  RequestLogTableCells,
  requestLogGridClass,
} from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";
import { AnchoredVirtualRows } from "@/shared/ui/AnchoredVirtualRows";
import {
  listEntrySurfaceAnimationClass,
  type ListEntryAnimation,
} from "@/shared/ui/useListEntryAnimations";

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
  const itemIds = useMemo(() => items.map((item) => item.requestId), [items]);

  return (
    <div className="hidden h-full min-h-0 overflow-x-auto md:block [scrollbar-gutter:stable]">
      <div role="table" aria-label="请求日志表格" aria-rowcount={items.length + 1} className="flex h-full min-w-[80rem] flex-col">
        <div role="rowgroup" aria-label="请求日志表头" className="shrink-0 overflow-y-scroll border-b border-subtle [scrollbar-gutter:stable]">
          <div role="row" aria-rowindex={1} className={cn(requestLogGridClass, "text-[11px] font-medium text-tertiary")}>
            <Header>时间</Header>
            <Header>客户端 IP</Header>
            <Header>令牌</Header>
            <Header>模型</Header>
            <Header>流式</Header>
            <Header>思考</Header>
            <Header>结果</Header>
            <Header>总耗时</Header>
            <Header>首字</Header>
            <Header>输入</Header>
            <Header>缓存命中</Header>
            <Header>输出</Header>
            <Header>TPS</Header>
          </div>
        </div>
        <AnchoredVirtualRows
          itemIds={itemIds}
          rowHeight={REQUEST_LOG_ROW_HEIGHT}
          followingLatest={followingLatest}
          hasMore={hasMore}
          loadingMore={loadingMore}
          historyLoaderKey="request-log-history-loader"
          initialWidth={1_120}
          ariaLabel="请求日志表格数据"
          onFollowingLatestChange={onFollowingLatestChange}
          onLoadMore={onLoadMore}
          renderRow={(index) => {
            const item = items[index]!;
            return (
              <RequestLogFeedTableRow
                item={item}
                selected={selectedId === item.requestId}
                nowMs={nowMs}
                animation={entryAnimations?.get(item.requestId)}
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

function RequestLogFeedTableRow({
  item,
  selected,
  nowMs,
  animation,
  onSelect,
}: {
  item: RequestLogFeedItem;
  selected: boolean;
  nowMs: number;
  animation: ListEntryAnimation | undefined;
  onSelect: (requestId: string) => void;
}) {
  const active = isActiveRequestLog(item);
  const settling = animation === "complete";
  const model = item.publicModel?.trim() || "未解析模型";
  return (
    <div
      role="row"
      tabIndex={active ? -1 : 0}
      aria-selected={active ? undefined : selected}
      aria-label={active ? undefined : `查看请求 ${model}`}
      title={active ? undefined : "双击查看详情"}
      className={cn(
        requestLogGridClass,
        "compact-row-surface relative h-11 rounded-[8px] text-[12px]",
        (active || settling) && "log-entry-surface-processing",
        active
          ? undefined
          : "compact-row-surface-hover focus-ring cursor-pointer outline-none",
        !active && selected && "compact-row-surface-selected",
        listEntrySurfaceAnimationClass(animation),
      )}
      onDoubleClick={active ? undefined : () => onSelect(item.requestId)}
      onKeyDown={active ? undefined : (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onSelect(item.requestId);
        }
      }}
    >
      <RequestAttemptMarker attemptCount={item.attemptCount} />
      {isActiveRequestLog(item) ? (
        <ActiveRequestLogTableCells log={item} nowMs={nowMs} />
      ) : (
        <RequestLogTableCells log={item} />
      )}
    </div>
  );
}

function Header({ children }: { children: ReactNode }) {
  return <div role="columnheader" className="min-w-0 whitespace-nowrap px-1 py-2 text-left">{children}</div>;
}
