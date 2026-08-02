import { RefreshCw, ScrollText } from "lucide-react";
import { useState, type ReactNode } from "react";

import { getRequestLogErrorMessage } from "../model/request-log-error";
import { useRequestLogs } from "../model/use-request-logs";
import {
  RequestLogCard,
  RequestLogTableRows,
  requestLogGridClass,
} from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";
import { logPageCount, type LogPageSize } from "@/shared/lib/log-pagination";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { LogPagination } from "@/shared/ui/LogPagination";
import { Surface } from "@/shared/ui/Surface";

export function RequestLogManagement() {
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<LogPageSize>(20);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const query = useRequestLogs(page, pageSize);

  const items = query.data?.items ?? [];
  const total = query.data?.total ?? 0;
  const totalPages = logPageCount(total, pageSize);
  const safePage = Math.min(Math.max(1, page), totalPages);
  const visibleExpandedId = items.some((item) => item.requestId === expandedId)
    ? expandedId
    : null;

  async function refreshLogs() {
    const result = await query.refetch();
    if (result.isSuccess) {
      notify.success("请求日志已刷新");
    }
  }

  const handlePageChange = (nextPage: number) => {
    setExpandedId(null);
    setPage(nextPage);
  };

  const handlePageSizeChange = (size: LogPageSize) => {
    setExpandedId(null);
    setPageSize(size);
    setPage(1);
  };

  if (query.isPending && !query.data) {
    return (
      <Surface
        className="flex min-h-56 items-center justify-center p-7 text-sm text-secondary"
        aria-busy="true"
      >
        正在读取请求日志
      </Surface>
    );
  }

  if (!query.data) {
    return (
      <Surface className="p-6" role="alert">
        <p className="font-semibold">无法读取请求日志</p>
        <p className="mt-2 text-sm text-secondary">{getRequestLogErrorMessage(query.error)}</p>
        <Button className="mt-5" onClick={() => void refreshLogs()} disabled={query.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </Surface>
    );
  }

  return (
    // Cap height to the main panel so accordion expand only grows the middle scroller,
    // never stretches the page and leaves empty space under the list.
    <div
      className="flex h-full min-h-0 flex-1 flex-col overflow-hidden"
      aria-busy={query.isFetching}
    >
      {/* Fixed top chrome: stats left, actions right */}
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-b border-subtle pb-3">
        <p className="text-[12px] text-secondary">
          队列{" "}
          <span className="tabular-nums text-primary">
            {query.data.telemetry.queuedRecords}
          </span>
          <span className="mx-1.5 text-tertiary">·</span>
          丢弃{" "}
          <span className="tabular-nums text-primary">
            {query.data.telemetry.droppedRecords}
          </span>
        </p>
        <Button variant="ghost" onClick={() => void refreshLogs()} disabled={query.isFetching}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          刷新
        </Button>
      </div>

      {query.isError ? (
        <Surface className="mt-3 shrink-0 border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      {/* List area fills remaining height; mobile/desktop each own their scroller (like system logs). */}
      <div className="min-h-0 flex-1 pt-3">
        {total === 0 ? (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
            <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">还没有请求日志</p>
            <p className="mt-1 text-[12px] text-secondary">
              通过网关完成一次请求后，记录会出现在这里。
            </p>
          </div>
        ) : (
          <>
            {/* Mobile: adaptive borderless cards */}
            <div
              className="management-scroll-viewport h-full space-y-2 overflow-y-auto md:hidden [scrollbar-gutter:stable]"
              role="list"
              aria-label="请求日志列表"
            >
              {items.map((log) => (
                <div key={log.requestId} role="listitem">
                  <RequestLogCard
                    log={log}
                    expanded={visibleExpandedId === log.requestId}
                    onToggle={() =>
                      setExpandedId((current) =>
                        current === log.requestId ? null : log.requestId,
                      )
                    }
                  />
                </div>
              ))}
            </div>

            {/* Desktop: fixed header + independent body scroll (same pattern as system logs). */}
            <div className="hidden h-full min-h-0 overflow-x-auto md:block [scrollbar-gutter:stable]">
              <div
                role="table"
                aria-label="请求日志表格"
                aria-rowcount={items.length + 1}
                className="flex h-full min-w-[52rem] flex-col"
              >
                <div
                  role="rowgroup"
                  aria-label="请求日志表头"
                  className="shrink-0 overflow-y-scroll border-b border-subtle bg-transparent [scrollbar-gutter:stable]"
                >
                  <div
                    role="row"
                    aria-rowindex={1}
                    className={cn(
                      requestLogGridClass,
                      "text-[11px] font-medium text-tertiary",
                    )}
                  >
                    <RequestLogHeader>
                      {/* Match body row chevron gutter so labels line up with values. */}
                      <span className="inline-flex min-w-0 items-center gap-0.5">
                        <span className="size-5 shrink-0" aria-hidden="true" />
                        时间
                      </span>
                    </RequestLogHeader>
                    <RequestLogHeader>令牌</RequestLogHeader>
                    <RequestLogHeader>模型</RequestLogHeader>
                    <RequestLogHeader>思考</RequestLogHeader>
                    <RequestLogHeader>结果</RequestLogHeader>
                    <RequestLogHeader>首字</RequestLogHeader>
                    <RequestLogHeader>总耗时</RequestLogHeader>
                    <RequestLogHeader>输入</RequestLogHeader>
                    <RequestLogHeader>输出</RequestLogHeader>
                    <RequestLogHeader>命中</RequestLogHeader>
                    <RequestLogHeader>TPS</RequestLogHeader>
                  </div>
                </div>
                <div
                  role="rowgroup"
                  aria-label="请求日志表格数据"
                  // Scrollable rowgroup must be keyboard-focusable.
                  // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
                  tabIndex={0}
                  className="focus-ring min-h-0 flex-1 overflow-y-scroll bg-transparent outline-none [scrollbar-gutter:stable]"
                >
                  {items.map((log) => (
                    <RequestLogTableRows
                      key={log.requestId}
                      log={log}
                      expanded={visibleExpandedId === log.requestId}
                      onToggle={() =>
                        setExpandedId((current) =>
                          current === log.requestId ? null : log.requestId,
                        )
                      }
                    />
                  ))}
                </div>
              </div>
            </div>
          </>
        )}
      </div>

      {/* Fixed bottom chrome: pagination only (total is already in the control). */}
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-subtle pt-3 text-[12px] text-secondary">
        <LogPagination
          page={safePage}
          pageSize={pageSize}
          total={total}
          onPageChange={handlePageChange}
          onPageSizeChange={handlePageSizeChange}
        />
      </div>
    </div>
  );
}

function RequestLogHeader({ children }: { children: ReactNode }) {
  return (
    <div role="columnheader" className="min-w-0 px-1 py-2 text-left">
      {children}
    </div>
  );
}
