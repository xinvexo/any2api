import {
  observeElementRect as observeVirtualElementRect,
  useVirtualizer,
} from "@tanstack/react-virtual";
import { useRef, type ReactNode } from "react";

import type { SystemLog } from "../api/system-log-contracts";
import {
  formatBytes,
  formatDuration,
  formatSystemLogTime,
  outcomeLabel,
  statusTone,
} from "../model/system-log-presentation";
import { cn } from "@/shared/lib/cn";

const ESTIMATED_ROW_HEIGHT = 41;
const gridClass =
  "grid grid-cols-[9.5rem_8rem_4.5rem_minmax(16rem,1fr)_4rem_5.5rem_5rem_5.5rem_5rem] items-start gap-2 px-1";

export function SystemLogVirtualTable({ items }: { items: readonly SystemLog[] }) {
  const viewportRef = useRef<HTMLDivElement>(null);
  // TanStack Virtual exposes a mutable controller that React Compiler must not memoize.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer<HTMLDivElement, HTMLDivElement>({
    count: items.length,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    getItemKey: (index) => items[index]?.requestId ?? index,
    overscan: 8,
    useFlushSync: false,
    initialRect: { width: 980, height: 640 },
    observeElementRect: (instance, callback) =>
      observeVirtualElementRect(instance, (rect) =>
        callback({
          width: rect.width,
          height: rect.height > 0 ? rect.height : 640,
        }),
      ),
    measureElement: (element, entry) => {
      const measured = entry?.borderBoxSize?.[0]?.blockSize
        ?? element.getBoundingClientRect().height;
      return measured > 0 ? measured : ESTIMATED_ROW_HEIGHT;
    },
  });

  return (
    <div className="hidden h-full min-h-0 overflow-x-auto md:block [scrollbar-gutter:stable]">
      <div
        role="table"
        aria-label="系统日志表格"
        aria-rowcount={items.length + 1}
        className="flex h-full min-w-[980px] flex-col"
      >
        <div
          role="rowgroup"
          aria-label="系统日志表头"
          className="shrink-0 overflow-y-auto border-b border-subtle bg-surface [scrollbar-gutter:stable]"
        >
          <div role="row" aria-rowindex={1} className={cn(gridClass, "text-[11px] font-medium text-tertiary")}>
            <Header>时间</Header>
            <Header>客户端</Header>
            <Header>方法</Header>
            <Header>请求路径</Header>
            <Header>状态</Header>
            <Header>协议</Header>
            <Header>耗时</Header>
            <Header>响应</Header>
            <Header>结果</Header>
          </div>
        </div>
        <div
          ref={viewportRef}
          role="rowgroup"
          aria-label="系统日志表格数据"
          // This scrollable rowgroup must be keyboard-focusable.
          // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
          tabIndex={0}
          className="focus-ring min-h-0 flex-1 overflow-y-auto bg-surface outline-none [scrollbar-gutter:stable]"
        >
          <div className="relative w-full" style={{ height: `${virtualizer.getTotalSize()}px` }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const log = items[virtualRow.index];
              return (
                <div
                  key={virtualRow.key}
                  ref={virtualizer.measureElement}
                  data-index={virtualRow.index}
                  role="row"
                  aria-rowindex={virtualRow.index + 2}
                  title={`Request ID: ${log.requestId} · Config revision: ${log.configRevision}`}
                  className={cn(
                    gridClass,
                    "absolute left-0 top-0 w-full border-b border-subtle/70 bg-surface text-[12px] hover:bg-surface-muted",
                  )}
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  <Cell className="tabular-nums text-secondary">
                    {formatSystemLogTime(log.startedAtMs)}
                  </Cell>
                  <Cell className="font-mono text-secondary">{log.clientIp ?? "未知"}</Cell>
                  <Cell className="font-mono font-semibold">{log.method}</Cell>
                  <Cell className="break-all font-mono leading-5">{log.path}</Cell>
                  <Cell className={cn("font-mono font-semibold", statusTone(log))}>
                    {log.statusCode ?? "-"}
                  </Cell>
                  <Cell className="font-mono text-secondary">{log.httpVersion}</Cell>
                  <Cell className="tabular-nums text-secondary">{formatDuration(log.durationMs)}</Cell>
                  <Cell className="tabular-nums text-secondary">{formatBytes(log.responseBytes)}</Cell>
                  <Cell className="text-secondary">{outcomeLabel(log.outcome)}</Cell>
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
  return <div role="columnheader" className="min-w-0 px-1 py-2 text-left">{children}</div>;
}

function Cell({ children, className }: { children: ReactNode; className?: string }) {
  return <div role="cell" className={cn("min-w-0 px-1 py-2.5", className)}>{children}</div>;
}
