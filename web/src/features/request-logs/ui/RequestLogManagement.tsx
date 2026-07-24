import { ChevronLeft, ChevronRight, RefreshCw, ScrollText } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  isRequestLogPageSize,
  paginateItems,
  REQUEST_LOG_PAGE_SIZE_OPTIONS,
  type RequestLogPageSize,
} from "../model/request-log-pagination";
import { getRequestLogErrorMessage } from "../model/request-log-error";
import { useRequestLogs } from "../model/use-request-logs";
import { RequestLogCard, RequestLogTableRows } from "./RequestLogTableRow";
import { selectClass } from "@/shared/ui/form-control";
import { Button } from "@/shared/ui/Button";
import { IconButton } from "@/shared/ui/IconButton";
import { Surface } from "@/shared/ui/Surface";

export function RequestLogManagement() {
  const query = useRequestLogs();
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<RequestLogPageSize>(20);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  const items = query.data?.items ?? [];
  const total = items.length;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const safePage = Math.min(Math.max(1, page), totalPages);
  const pageItems = useMemo(
    () => paginateItems(items, safePage, pageSize),
    [items, safePage, pageSize],
  );

  useEffect(() => {
    if (page !== safePage) {
      setPage(safePage);
    }
  }, [page, safePage]);

  useEffect(() => {
    // Collapse accordion when the expanded row leaves the current page.
    if (expandedId && !pageItems.some((item) => item.requestId === expandedId)) {
      setExpandedId(null);
    }
  }, [expandedId, pageItems]);

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
    <div aria-busy={query.isFetching}>
      <div className="flex flex-col gap-2.5 border-b border-subtle pb-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
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
        <RequestLogPagination
          page={safePage}
          pageSize={pageSize}
          total={total}
          onPageChange={setPage}
          onPageSizeChange={(size) => {
            setPageSize(size);
            setPage(1);
          }}
        />
      </div>

      {query.isError ? (
        <Surface className="mt-3 border-warning/40 p-4 text-sm text-secondary" role="status">
          刷新失败，当前仍显示最近一次有效数据：{getRequestLogErrorMessage(query.error)}
        </Surface>
      ) : null}

      {total === 0 ? (
        <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
          <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-[13px] font-medium">还没有请求日志</p>
          <p className="mt-1 text-[12px] text-secondary">
            通过网关完成一次 Codex 或 Claude 请求后，记录会出现在这里。
          </p>
        </div>
      ) : (
        <>
          {/* Mobile: adaptive borderless cards */}
          <div
            className="space-y-2 pt-3 md:hidden"
            role="list"
            aria-label="请求日志列表"
          >
            {pageItems.map((log) => (
              <div key={log.requestId} role="listitem">
                <RequestLogCard
                  log={log}
                  expanded={expandedId === log.requestId}
                  onToggle={() =>
                    setExpandedId((current) =>
                      current === log.requestId ? null : log.requestId,
                    )
                  }
                />
              </div>
            ))}
          </div>

          {/* Desktop: normal data table */}
          <div className="hidden overflow-x-auto pt-3 md:block">
            <table className="w-full min-w-[44rem] border-collapse text-left" aria-label="请求日志表格">
              <thead>
                <tr className="border-b border-subtle text-[11px] font-medium text-tertiary">
                  <th scope="col" className="px-2 py-2 font-medium">
                    模型
                  </th>
                  <th scope="col" className="px-2 py-2 font-medium">
                    时间
                  </th>
                  <th scope="col" className="px-2 py-2 font-medium">
                    来源
                  </th>
                  <th scope="col" className="px-2 py-2 font-medium">
                    结果
                  </th>
                  <th scope="col" className="px-2 py-2 font-medium">
                    首字
                  </th>
                  <th scope="col" className="px-2 py-2 font-medium">
                    总延迟
                  </th>
                  <th scope="col" className="px-2 py-2 text-right font-medium">
                    Token
                  </th>
                </tr>
              </thead>
              <tbody>
                {pageItems.map((log) => (
                  <RequestLogTableRows
                    key={log.requestId}
                    log={log}
                    expanded={expandedId === log.requestId}
                    onToggle={() =>
                      setExpandedId((current) =>
                        current === log.requestId ? null : log.requestId,
                      )
                    }
                  />
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
        <p>
          共 <span className="tabular-nums">{total}</span> 条
          {total > 0 ? (
            <>
              <span className="mx-1.5 text-tertiary">·</span>
              本页 <span className="tabular-nums">{pageItems.length}</span> 条
            </>
          ) : null}
        </p>
        <RequestLogPagination
          page={safePage}
          pageSize={pageSize}
          total={total}
          onPageChange={setPage}
          onPageSizeChange={(size) => {
            setPageSize(size);
            setPage(1);
          }}
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
