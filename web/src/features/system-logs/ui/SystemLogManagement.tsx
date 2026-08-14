import { RefreshCw, ScrollText, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";

import {
  loadSystemLogAutoRefreshPreference,
  saveSystemLogAutoRefreshPreference,
} from "../model/system-log-auto-refresh-preference";
import { useClearSystemLogs, useSystemLogs } from "../model/use-system-logs";
import { SystemLogList } from "./SystemLogList";
import { SystemLogDetailDrawer } from "./SystemLogDetailDrawer";
import type { LogPageSize } from "@/shared/lib/log-pagination";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { LogPagination } from "@/shared/ui/LogPagination";
import { Switch } from "@/shared/ui/Switch";

export function SystemLogManagement() {
  const [autoRefresh, setAutoRefresh] = useState(loadSystemLogAutoRefreshPreference);
  const [confirmClear, setConfirmClear] = useState(false);
  const [{ page, cursor }, setLocation] = useState({ page: 1, cursor: null as string | null });
  const [pageSize, setPageSize] = useState<LogPageSize>(20);
  const [selectedRequestId, setSelectedRequestId] = useState<string | null>(null);
  const query = useSystemLogs(autoRefresh, cursor, page, pageSize);
  const clearMutation = useClearSystemLogs();
  const total = query.data?.total ?? 0;
  const displayedPage = query.isPlaceholderData ? page : (query.data?.page ?? page);

  useEffect(() => {
    if (query.isPlaceholderData || !query.data || query.data.page === page) {
      return;
    }

    const correction = window.setTimeout(() => {
      setSelectedRequestId(null);
      setLocation({ page: query.data.page, cursor: query.data.cursor });
    }, 0);

    return () => window.clearTimeout(correction);
  }, [page, query.data, query.isPlaceholderData]);

  async function refreshLogs() {
    if (cursor !== null || page !== 1) {
      setSelectedRequestId(null);
      setLocation({ page: 1, cursor: null });
      return;
    }
    const result = await query.refetch();
    if (result.isSuccess) {
      notify.success("系统日志已刷新");
    }
  }

  const handleAutoRefreshChange = (enabled: boolean) => {
    setAutoRefresh(enabled);
    saveSystemLogAutoRefreshPreference(enabled);
  };

  const handlePageChange = (nextPage: number) => {
    if (query.isPlaceholderData || !query.data || nextPage === query.data.page) {
      return;
    }

    const nextCursor =
      nextPage === query.data.page + 1 ? query.data.nextCursor : query.data.cursor;
    if (nextCursor === null) {
      return;
    }

    setSelectedRequestId(null);
    setLocation({ page: nextPage, cursor: nextCursor });
  };

  const handleClear = () => {
    clearMutation.mutate(undefined, {
      onSuccess: (result) => {
        setConfirmClear(false);
        setSelectedRequestId(null);
        setLocation({ page: 1, cursor: null });
        notify.success(`已清理 ${result.deleted} 条历史系统日志`);
      },
      onError: () => notify.danger("系统日志清理失败"),
    });
  };

  if (query.isPending && !query.data) {
    return (
      <div className="flex min-h-56 items-center justify-center text-sm text-secondary" aria-busy="true">
        正在读取系统日志
      </div>
    );
  }

  if (!query.data) {
    return (
      <div className="flex min-h-56 flex-col items-center justify-center text-center" role="alert">
        <p className="text-sm font-semibold">无法读取系统日志</p>
        <Button className="mt-4" onClick={() => void refreshLogs()} disabled={query.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </div>
    );
  }

  const items = query.data.items;

  return (
    <div
      className="flex flex-1 flex-col md:h-full md:min-h-0 md:overflow-hidden"
      aria-busy={query.isFetching}
    >
      <div
        data-system-log-fixed="toolbar"
        className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-subtle pb-3"
      >
        <div className="flex items-center gap-2 text-[12px] text-secondary">
          <span id="system-log-auto-refresh-label">自动刷新</span>
          <Switch
            id="system-log-auto-refresh"
            checked={autoRefresh}
            aria-labelledby="system-log-auto-refresh-label"
            onCheckedChange={handleAutoRefreshChange}
          />
        </div>
        <div className="flex items-center gap-0.5">
          <Button
            size="sm"
            variant="ghost"
            className="h-9 min-h-9 w-9 rounded-full px-0 md:h-7 md:min-h-7 md:w-auto md:rounded-[6px] md:px-2.5"
            onClick={() => void refreshLogs()}
            disabled={query.isFetching}
            title="刷新"
          >
            <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
            <span className="sr-only md:not-sr-only">刷新</span>
          </Button>
          <Button
            size="sm"
            variant="danger"
            className="h-9 min-h-9 w-9 rounded-full px-0 md:h-7 md:min-h-7 md:w-auto md:rounded-[6px] md:px-2.5"
            disabled={clearMutation.isPending}
            onClick={() => setConfirmClear(true)}
            title="清理历史日志"
          >
            <Trash2 size={14} />
            <span className="sr-only md:not-sr-only">清理历史日志</span>
          </Button>
        </div>
      </div>

      {query.isError ? (
        <p className="shrink-0 border-b border-warning/30 py-3 text-[12px] text-warning" role="status">
          刷新失败，当前显示最近一次有效数据
        </p>
      ) : null}

      <div className="pt-3 md:min-h-0 md:flex-1">
        {items.length === 0 ? (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
            <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">还没有系统日志</p>
          </div>
        ) : (
          <SystemLogList items={items} onSelect={setSelectedRequestId} />
        )}
      </div>

      <div
        data-system-log-fixed="pagination"
        className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-subtle pt-3"
      >
        <LogPagination
          page={displayedPage}
          pageSize={pageSize}
          total={total}
          hasNextPage={!query.isPlaceholderData && query.data.nextCursor !== null}
          disabled={query.isPlaceholderData}
          onPageChange={handlePageChange}
          onPageSizeChange={(size) => {
            setPageSize(size);
            setLocation({ page: 1, cursor: null });
          }}
        />
      </div>

      <ConfirmDialog
        open={confirmClear}
        title="清理历史系统日志？"
        description="将删除数据库中当前保留的全部系统日志，此操作不可撤销。"
        confirmLabel="清理"
        tone="danger"
        pending={clearMutation.isPending}
        onClose={() => setConfirmClear(false)}
        onConfirm={handleClear}
      />
      <SystemLogDetailDrawer
        requestId={selectedRequestId}
        onClose={() => setSelectedRequestId(null)}
      />
    </div>
  );
}
