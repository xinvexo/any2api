import { ChevronLeft, ChevronRight, RefreshCw, ScrollText } from "lucide-react";
import { useMemo, useState } from "react";

import {
  isRequestLogPageSize,
  paginateItems,
  REQUEST_LOG_PAGE_SIZE_OPTIONS,
  type RequestLogPageSize,
} from "../model/request-log-pagination";
import { getRequestLogErrorMessage } from "../model/request-log-error";
import { useRequestLogs } from "../model/use-request-logs";
import {
  RequestLogCard,
  RequestLogTableRows,
  requestLogGridClass,
} from "./RequestLogTableRow";
import { cn } from "@/shared/lib/cn";
import { selectClass } from "@/shared/ui/form-control";
import { Button } from "@/shared/ui/Button";
import { IconButton } from "@/shared/ui/IconButton";
import { Surface } from "@/shared/ui/Surface";

export function RequestLogManagement() {
  const query = useRequestLogs();
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<RequestLogPageSize>(20);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const items = useMemo(() => query.data?.items ?? [], [query.data?.items]);
  const total = items.length;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const safePage = Math.min(Math.max(1, page), totalPages);
  const pageItems = useMemo(
    () => paginateItems(items, safePage, pageSize),
    [items, safePage, pageSize],
  );
  const visibleExpandedId = pageItems.some((item) => item.requestId === expandedId)
    ? expandedId
    : null;

  const handlePageChange = (nextPage: number) => {
    setExpandedId(null);
    setPage(nextPage);
  };

  const handlePageSizeChange = (size: RequestLogPageSize) => {
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
        <Button className="mt-5" onClick={() => void query.refetch()} disabled={query.isFetching}>
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
      {/* Fixed top chrome: refresh + telemetry */}
      <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-subtle pb-3">
        <Button variant="ghost" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          刷新
        </Button>
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
      </div>

      {query.isError ? (
        <Surface className="mt-3 shrink-0 border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      {/* Only the list scrolls; toolbar and pagination stay put. */}
      <div className="min-h-0 flex-1 overflow-auto pt-3 [scrollbar-gutter:stable]">
        {total === 0 ? (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
            <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">还没有请求日志</p>
            <p className="mt-1 text-[12px] text-secondary">
              通过网关完成一次 Codex、Claude 或 Grok 请求后，记录会出现在这里。
            </p>
          </div>
        ) : (
          <>
            {/* Mobile: adaptive borderless cards */}
            <div className="space-y-2 md:hidden" role="list" aria-label="请求日志列表">
              {pageItems.map((log) => (
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

            {/* Full-width CSS grid: free space shared across columns (no dead right gutter). */}
            <div
              className="hidden md:block"
              role="table"
              aria-label="请求日志表格"
            >
              <div
                role="rowgroup"
                className="sticky top-0 z-10 border-b border-subtle bg-surface"
              >
                <div
                  role="row"
                  className={cn(
                    requestLogGridClass,
                    "text-[11px] font-medium text-tertiary",
                  )}
                >
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    时间
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    令牌
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    模型
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    思考
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    结果
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    首字
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    总耗时
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    入
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    出
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    命中
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    创建
                  </div>
                  <div role="columnheader" className="min-w-0 truncate px-1 py-2 text-left">
                    TPS
                  </div>
                </div>
              </div>
              {pageItems.map((log) => (
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
          </>
        )}
      </div>

      {/* Fixed bottom chrome: pagination only (total is already in the control). */}
      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-subtle pt-3 text-[12px] text-secondary">
        <RequestLogPagination
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

function RequestLogPagination({
  page,
  pageSize,
  total,
  onPageChange,
  onPageSizeChange,
}: {
  page: number;
  pageSize: RequestLogPageSize;
  total: number;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: RequestLogPageSize) => void;
}) {
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const safePage = Math.min(Math.max(1, page), totalPages);

  return (
    <div className="flex h-8 min-w-0 flex-wrap items-center gap-1.5 text-[12px] text-secondary">
      <label className="flex items-center gap-1.5">
        <span className="sr-only">每页条数</span>
        <select
          className={selectClass(false, "w-auto min-w-[4.5rem]")}
          value={pageSize}
          aria-label="每页条数"
          onChange={(event) => {
            const next = Number(event.target.value);
            if (isRequestLogPageSize(next)) {
              onPageSizeChange(next);
            }
          }}
        >
          {REQUEST_LOG_PAGE_SIZE_OPTIONS.map((size) => (
            <option key={size} value={size}>
              {size} 条/页
            </option>
          ))}
        </select>
      </label>
      <span className="tabular-nums text-tertiary">共 {total} 条</span>
      <div className="flex items-center gap-0.5">
        <IconButton
          label="上一页"
          disabled={safePage <= 1}
          onClick={() => onPageChange(safePage - 1)}
        >
          <ChevronLeft size={16} strokeWidth={1.75} />
        </IconButton>
        <span className="min-w-[3.25rem] text-center tabular-nums text-primary">
          {safePage}/{totalPages}
        </span>
        <IconButton
          label="下一页"
          disabled={safePage >= totalPages}
          onClick={() => onPageChange(safePage + 1)}
        >
          <ChevronRight size={16} strokeWidth={1.75} />
        </IconButton>
      </div>
    </div>
  );
}
