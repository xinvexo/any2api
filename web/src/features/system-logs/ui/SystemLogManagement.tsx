import { RefreshCw, ScrollText, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";

import type { SystemLog } from "../api/system-log-contracts";
import {
  loadSystemLogAdminOperationsPreference,
  saveSystemLogAdminOperationsPreference,
} from "../model/system-log-admin-operations-preference";
import { useClearSystemLogs } from "../model/use-clear-system-logs";
import { useSystemLogs } from "../model/use-system-logs";
import { SystemLogDetailDrawer } from "./SystemLogDetailDrawer";
import { SystemLogList } from "./SystemLogList";
import { notify } from "@/shared/notifications";
import { useAdminRealtimeStatus } from "@/shared/realtime";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { ScrollToTopButton } from "@/shared/ui/ScrollToTopButton";
import { Switch } from "@/shared/ui/Switch";
import { useListEntryAnimations } from "@/shared/ui/useListEntryAnimations";

export function SystemLogManagement() {
  const [showAdminOperations, setShowAdminOperations] = useState(loadSystemLogAdminOperationsPreference);
  const [followingLatest, setFollowingLatest] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const query = useSystemLogs(showAdminOperations, followingLatest);
  const clearMutation = useClearSystemLogs();
  const realtime = useAdminRealtimeStatus();
  const entryAnimations = useListEntryAnimations(
    query.items,
    systemLogEntryId,
    systemLogEntryState,
    `${showAdminOperations}\u0000${query.data ? "ready" : "loading"}`,
  );
  const { fetchNextPage, hasNextPage, isFetchingNextPage, refreshLatest } = query;

  const refreshLogs = useCallback(async () => {
    setFollowingLatest(true);
    try {
      await refreshLatest();
      notify.success("系统日志已刷新");
    } catch {
      notify.danger("系统日志刷新失败");
    }
  }, [refreshLatest]);

  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage();
  }, [fetchNextPage, hasNextPage, isFetchingNextPage]);

  const handleShowAdminOperationsChange = (enabled: boolean) => {
    setShowAdminOperations(enabled);
    saveSystemLogAdminOperationsPreference(enabled);
    setSelectedId(null);
    setFollowingLatest(true);
  };

  const handleClear = () => {
    clearMutation.mutate(undefined, {
      onSuccess: (result) => {
        setConfirmClear(false);
        setSelectedId(null);
        setFollowingLatest(true);
        notify.success(`已清理 ${result.deleted} 条历史系统日志`);
      },
      onError: () => notify.danger("系统日志清理失败"),
    });
  };

  return (
    <div className="flex flex-1 flex-col md:h-full md:min-h-0 md:overflow-hidden" aria-busy={query.isFetching}>
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-subtle pb-3">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[12px] text-secondary">
          <Toggle id="system-log-admin-operations" label="显示管理操作" checked={showAdminOperations} onChange={handleShowAdminOperationsChange} />
          {!realtime.connected ? <span className="text-warning">实时连接中断</span> : null}
        </div>
        <div className="flex items-center gap-0.5">
          <Button size="sm" variant="ghost" className="h-9 min-h-9 w-9 rounded-full px-0 md:h-7 md:min-h-7 md:w-auto md:rounded-[6px] md:px-2.5" onClick={() => void refreshLogs()} disabled={query.isFetching && !query.isFetchingNextPage} title="刷新">
            <RefreshCw size={14} className={query.isFetching && !query.isFetchingNextPage ? "animate-spin" : undefined} />
            <span className="sr-only md:not-sr-only">刷新</span>
          </Button>
          <Button size="sm" variant="danger" className="h-9 min-h-9 w-9 rounded-full px-0 md:h-7 md:min-h-7 md:w-auto md:rounded-[6px] md:px-2.5" disabled={clearMutation.isPending} onClick={() => setConfirmClear(true)} title="清理历史日志">
            <Trash2 size={14} /><span className="sr-only md:not-sr-only">清理历史日志</span>
          </Button>
        </div>
      </div>

      {query.isError && query.data ? <p className="shrink-0 border-b border-warning/30 py-3 text-[12px] text-warning" role="status">同步失败，当前显示最近一次有效数据</p> : null}

      {query.data ? (
        <div className="pt-3 md:min-h-0 md:flex-1">
          {query.items.length === 0 ? (
            <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center"><ScrollText size={22} className="text-tertiary" /><p className="mt-3 text-[13px] font-medium">还没有系统日志</p></div>
          ) : (
            <SystemLogList
              items={query.items}
              selectedId={selectedId}
              followingLatest={followingLatest}
              hasMore={query.hasNextPage}
              loadingMore={query.isFetchingNextPage}
              onSelect={setSelectedId}
              onFollowingLatestChange={setFollowingLatest}
              onLoadMore={loadMore}
              entryAnimations={entryAnimations}
            />
          )}
        </div>
      ) : query.isPending ? (
        <div className="flex min-h-56 flex-1 items-center justify-center text-sm text-secondary" aria-busy="true">正在读取系统日志</div>
      ) : (
        <div className="flex min-h-56 flex-1 flex-col items-center justify-center text-center" role="alert"><p className="text-sm font-semibold">无法读取系统日志</p><Button className="mt-4" onClick={() => void refreshLogs()}><RefreshCw size={15} />重试</Button></div>
      )}

      <ScrollToTopButton
        visible={!followingLatest}
        onClick={() => {
          setFollowingLatest(true);
          query.applyPending();
        }}
      />

      <ConfirmDialog open={confirmClear} title="清理历史系统日志？" description="将删除数据库中当前保留的全部系统日志，此操作不可撤销。" confirmLabel="清理" tone="danger" pending={clearMutation.isPending} onClose={() => setConfirmClear(false)} onConfirm={handleClear} />
      <SystemLogDetailDrawer requestId={selectedId} onClose={() => setSelectedId(null)} />
    </div>
  );
}

function Toggle({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (checked: boolean) => void }) {
  return <div className="flex items-center gap-2"><span id={`${id}-label`}>{label}</span><Switch id={id} checked={checked} aria-labelledby={`${id}-label`} onCheckedChange={onChange} /></div>;
}

function systemLogEntryId(log: SystemLog) {
  return log.requestId;
}

function systemLogEntryState(log: SystemLog) {
  return log.outcome;
}
