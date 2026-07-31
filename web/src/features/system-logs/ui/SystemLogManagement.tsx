import { RefreshCw, ScrollText, Trash2 } from "lucide-react";
import { useState } from "react";

import {
  loadSystemLogAutoRefreshPreference,
  saveSystemLogAutoRefreshPreference,
} from "../model/system-log-auto-refresh-preference";
import { useClearSystemLogs, useSystemLogs } from "../model/use-system-logs";
import { SystemLogList } from "./SystemLogList";
import { logPageCount, type LogPageSize } from "@/shared/lib/log-pagination";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { LogPagination } from "@/shared/ui/LogPagination";
import { Switch } from "@/shared/ui/Switch";

export function SystemLogManagement() {
  const [autoRefresh, setAutoRefresh] = useState(loadSystemLogAutoRefreshPreference);
  const [confirmClear, setConfirmClear] = useState(false);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState<LogPageSize>(20);
  const query = useSystemLogs(autoRefresh, page, pageSize);
  const clearMutation = useClearSystemLogs();
  const total = query.data?.total ?? 0;
  const totalPages = logPageCount(total, pageSize);
  const safePage = Math.min(Math.max(1, page), totalPages);

  const handleAutoRefreshChange = (enabled: boolean) => {
    setAutoRefresh(enabled);
    saveSystemLogAutoRefreshPreference(enabled);
  };

  const handleClear = () => {
    clearMutation.mutate(undefined, {
      onSuccess: (result) => {
        setConfirmClear(false);
        setPage(1);
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
        <Button className="mt-4" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={15} />
          重试
        </Button>
      </div>
    );
  }

  const items = query.data.items;

  return (
    <div className="flex h-full min-h-0 flex-1 flex-col overflow-hidden" aria-busy={query.isFetching}>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-subtle pb-3">
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
            onClick={() => void query.refetch()}
            disabled={query.isFetching}
          >
            <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button
            size="sm"
            variant="danger"
            disabled={clearMutation.isPending}
            onClick={() => setConfirmClear(true)}
          >
            <Trash2 size={14} />
            清理历史日志
          </Button>
        </div>
      </div>

      {query.isError ? (
        <p className="shrink-0 border-b border-warning/30 py-3 text-[12px] text-warning" role="status">
          刷新失败，当前显示最近一次有效数据
        </p>
      ) : null}

      <div className="min-h-0 flex-1">
        {items.length === 0 ? (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
            <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">还没有系统日志</p>
          </div>
        ) : (
          <SystemLogList items={items} />
        )}
      </div>

      <div className="flex shrink-0 flex-wrap items-center justify-end gap-2 border-t border-subtle pt-3">
        <LogPagination
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
    </div>
  );
}
