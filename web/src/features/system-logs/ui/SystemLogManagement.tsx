import { RefreshCw, ScrollText, Trash2 } from "lucide-react";
import { useState } from "react";

import {
  loadSystemLogAutoRefreshPreference,
  saveSystemLogAutoRefreshPreference,
} from "../model/system-log-auto-refresh-preference";
import { useClearSystemLogs, useSystemLogs } from "../model/use-system-logs";
import { SystemLogList } from "./SystemLogList";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { Switch } from "@/shared/ui/Switch";

export function SystemLogManagement() {
  const [autoRefresh, setAutoRefresh] = useState(loadSystemLogAutoRefreshPreference);
  const [confirmClear, setConfirmClear] = useState(false);
  const query = useSystemLogs(autoRefresh);
  const clearMutation = useClearSystemLogs();

  const handleAutoRefreshChange = (enabled: boolean) => {
    setAutoRefresh(enabled);
    saveSystemLogAutoRefreshPreference(enabled);
  };

  const handleClear = () => {
    clearMutation.mutate(undefined, {
      onSuccess: (result) => {
        setConfirmClear(false);
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
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-subtle pb-3">
        <Button variant="ghost" onClick={() => void query.refetch()} disabled={query.isFetching}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          刷新
        </Button>
        <div className="flex items-center gap-2 text-[12px] text-secondary">
          <span id="system-log-auto-refresh-label">自动刷新</span>
          <Switch
            id="system-log-auto-refresh"
            checked={autoRefresh}
            aria-labelledby="system-log-auto-refresh-label"
            onCheckedChange={handleAutoRefreshChange}
          />
        </div>
        <Button
          className="ml-auto"
          variant="danger"
          disabled={items.length === 0 || clearMutation.isPending}
          onClick={() => setConfirmClear(true)}
        >
          <Trash2 size={14} />
          清理历史日志
        </Button>
        <p className="w-full text-[11px] text-tertiary sm:w-auto">
          最近 {items.length} 条 · 队列 {query.data.telemetry.queuedRecords} · 丢弃 {query.data.telemetry.droppedRecords}
        </p>
      </div>

      {query.isError ? (
        <p className="shrink-0 border-b border-warning/30 py-3 text-[12px] text-warning" role="status">
          刷新失败，当前显示最近一次有效数据
        </p>
      ) : null}

      <div className="min-h-0 flex-1 pt-3">
        {items.length === 0 ? (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center">
            <ScrollText size={22} className="text-tertiary" aria-hidden="true" />
            <p className="mt-3 text-[13px] font-medium">还没有系统日志</p>
          </div>
        ) : (
          <SystemLogList items={items} />
        )}
      </div>

      <ConfirmDialog
        open={confirmClear}
        title="清理历史系统日志？"
        description={`将删除当前保留的 ${items.length} 条记录，此操作不可撤销。清理操作本身会作为一条新记录保留。`}
        confirmLabel="清理"
        tone="danger"
        pending={clearMutation.isPending}
        onClose={() => setConfirmClear(false)}
        onConfirm={handleClear}
      />
    </div>
  );
}
