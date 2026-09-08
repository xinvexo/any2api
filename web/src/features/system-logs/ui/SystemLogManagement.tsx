import { RefreshCw, ScrollText, Search, Trash2 } from "lucide-react";
import { useCallback, useState } from "react";

import type { SystemLog, SystemLogFilters } from "../api/system-log-contracts";
import {
  loadSystemLogAdminOperationsPreference,
  saveSystemLogAdminOperationsPreference,
} from "../model/system-log-admin-operations-preference";
import { useClearSystemLogs } from "../model/use-clear-system-logs";
import { useSystemLogs } from "../model/use-system-logs";
import { SystemLogDetailDrawer } from "./SystemLogDetailDrawer";
import { SystemLogList } from "./SystemLogList";
import { notify } from "@/shared/notifications";
import { useAdminRealtimeReconnect, useAdminRealtimeStatus } from "@/shared/realtime";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";
import { ScrollToTopButton } from "@/shared/ui/ScrollToTopButton";
import { Switch } from "@/shared/ui/Switch";
import { useListEntryAnimations } from "@/shared/ui/useListEntryAnimations";
import { controlClass } from "@/shared/ui/form-control";

export function SystemLogManagement() {
  const [showAdminOperations, setShowAdminOperations] = useState(loadSystemLogAdminOperationsPreference);
  const [followingLatest, setFollowingLatest] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filters, setFilters] = useState<SystemLogFilters>({});
  const [draft, setDraft] = useState({ statusCode: "", clientIp: "", path: "" });
  const query = useSystemLogs(showAdminOperations, followingLatest, filters);
  const clearMutation = useClearSystemLogs();
  const realtime = useAdminRealtimeStatus();
  const reconnect = useAdminRealtimeReconnect();
  const entryAnimations = useListEntryAnimations(
    query.items,
    systemLogEntryId,
    systemLogEntryState,
    `${JSON.stringify([showAdminOperations, filters])}\u0000${query.data ? "ready" : "loading"}`,
  );
  const { fetchNextPage, hasNextPage, isFetchingNextPage, refreshLatest } = query;

  const refreshLogs = useCallback(async () => {
    setFollowingLatest(true);
    try {
      if (!await refreshLatest()) {
        return;
      }
      if (!realtime.connected) reconnect();
      notify.success("HTTP 访问日志已刷新");
    } catch {
      notify.danger("HTTP 访问日志刷新失败");
    }
  }, [reconnect, realtime.connected, refreshLatest]);

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
        notify.success(`已清空 ${result.deleted} 条 HTTP 访问日志`);
      },
      onError: () => notify.danger("清空 HTTP 访问日志失败"),
    });
  };

  return (
    <div className="flex min-w-0 flex-1 flex-col md:h-full md:min-h-0 md:overflow-hidden" aria-busy={query.isFetching}>
      <form aria-label="HTTP 访问筛选" className="mb-3 grid shrink-0 grid-cols-2 gap-2 lg:flex" onSubmit={(event) => {
        event.preventDefault();
        setFilters({ statusCode: draft.statusCode || undefined, clientIp: draft.clientIp.trim() || undefined, path: draft.path.trim() || undefined });
        setFollowingLatest(true);
        setSelectedId(null);
      }}>
        <input type="number" min={100} max={599} inputMode="numeric" aria-label="HTTP 状态码" placeholder="状态码，如 401" value={draft.statusCode} onChange={(event) => setDraft({ ...draft, statusCode: event.target.value })} className={controlClass(false, "lg:w-36")} />
        <input aria-label="客户端 IP 筛选" placeholder="客户端 IP" value={draft.clientIp} onChange={(event) => setDraft({ ...draft, clientIp: event.target.value })} className={controlClass(false, "lg:w-40")} />
        <div className="col-span-2 flex min-w-0 flex-1 gap-2">
          <input aria-label="请求路径筛选" placeholder="搜索请求路径" maxLength={256} value={draft.path} onChange={(event) => setDraft({ ...draft, path: event.target.value })} className={controlClass(false, "min-w-0 flex-1")} />
          <Button type="submit"><Search size={14} />筛选</Button>
          <Button variant="ghost" onClick={() => { setDraft({ statusCode: "", clientIp: "", path: "" }); setFilters({}); setFollowingLatest(true); setSelectedId(null); }}>重置</Button>
        </div>
      </form>
      <header className="flex min-h-8 shrink-0 flex-wrap items-center justify-between gap-3 border-b border-subtle pb-3">
        <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[12px] text-secondary">
          <Toggle id="system-log-admin-operations" label="显示管理操作" checked={showAdminOperations} onChange={handleShowAdminOperationsChange} />
          {query.items.length ? <span>已加载 {query.items.length} 条</span> : null}
          {!realtime.connected ? <span className="text-warning">实时连接中断</span> : null}
        </div>
        <div className="flex items-center gap-2">
          <Button size="lg" variant="ghost" className="h-9 min-h-9 w-9 rounded-full px-0 md:h-8 md:min-h-8 md:w-auto md:rounded-[7px] md:px-3.5" onClick={() => void refreshLogs()} disabled={query.isFetching && !query.isFetchingNextPage} title="刷新">
            <RefreshCw size={14} className={query.isFetching && !query.isFetchingNextPage ? "animate-spin" : undefined} />
            <span className="sr-only md:not-sr-only">刷新</span>
          </Button>
          <Button size="lg" variant="danger" className="h-9 min-h-9 w-9 rounded-full px-0 md:h-8 md:min-h-8 md:w-auto md:rounded-[7px] md:px-3.5" disabled={clearMutation.isPending} onClick={() => setConfirmClear(true)} title="清空记录">
            <Trash2 size={14} /><span className="sr-only md:not-sr-only">清空记录</span>
          </Button>
        </div>
      </header>

      {query.isError && query.data ? <p className="shrink-0 border-b border-warning/30 py-3 text-[12px] text-warning" role="status">同步失败，当前显示最近一次有效数据</p> : null}

      {query.data ? (
        <div className="pt-3 md:min-h-0 md:flex-1">
          {query.items.length === 0 ? (
            <div className="flex min-h-48 flex-col items-center justify-center px-6 py-10 text-center"><ScrollText size={22} className="text-tertiary" /><p className="mt-3 text-[13px] font-medium">暂无符合条件的 HTTP 访问记录</p><p className="mt-2 text-[12px] text-secondary">可调整状态码、客户端 IP 或路径筛选。</p></div>
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

      <ConfirmDialog open={confirmClear} title="清空全部 HTTP 访问日志？" description="将删除当前保留的全部 HTTP 访问记录，此操作不可撤销。" confirmLabel="清空" tone="danger" pending={clearMutation.isPending} onClose={() => setConfirmClear(false)} onConfirm={handleClear} />
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
