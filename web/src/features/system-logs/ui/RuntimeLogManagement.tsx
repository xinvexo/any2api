import { useState } from "react";
import { ChevronRight, RefreshCw, Search, ScrollText } from "lucide-react";

import type { RuntimeLogFilters } from "../api/runtime-log-api";
import { eventReason, eventTitle, levelLabels, moduleLabels } from "../model/runtime-log-presentation";
import { useRuntimeLogs } from "../model/use-runtime-logs";
import { RuntimeLogDetailDrawer, RuntimeLogLevelBadge } from "./RuntimeLogDetailDrawer";
import type { RuntimeLogEntry } from "@/shared/api/generated/RuntimeLogEntry";
import type { RuntimeLogLevel } from "@/shared/api/generated/RuntimeLogLevel";
import { cn } from "@/shared/lib/cn";
import { formatCompactDateTime } from "@/shared/lib/date-time";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { AnchoredVirtualRows } from "@/shared/ui/AnchoredVirtualRows";
import { IntersectionSentinel } from "@/shared/ui/IntersectionSentinel";
import { ScrollToTopButton } from "@/shared/ui/ScrollToTopButton";
import { Select } from "@/shared/ui/Select";
import { WindowVirtualList } from "@/shared/ui/WindowVirtualList";
import { controlClass } from "@/shared/ui/form-control";
import { useMobileViewport } from "@/shared/ui/use-mobile-viewport";

export function RuntimeLogManagement() {
  const [filters, setFilters] = useState<RuntimeLogFilters>({});
  const [search, setSearch] = useState("");
  const [followingLatest, setFollowingLatest] = useState(true);
  const [selected, setSelected] = useState<RuntimeLogEntry | null>(null);
  const query = useRuntimeLogs(filters, followingLatest);
  const mobile = useMobileViewport();

  function changeFilters(next: RuntimeLogFilters) {
    setFilters(next);
    setFollowingLatest(true);
    setSelected(null);
  }

  async function refresh() {
    setFollowingLatest(true);
    const result = await query.refresh();
    if (result.isError) notify.danger("运行日志刷新失败");
    else notify.success("运行日志已刷新");
  }

  const loadMore = () => {
    if (query.hasNextPage && !query.isFetching) void query.fetchNextPage();
  };

  return <div className="flex min-w-0 flex-1 flex-col md:min-h-0">
    <form aria-label="运行日志筛选" className="grid shrink-0 grid-cols-2 gap-2 border-b border-subtle pb-3 lg:flex lg:flex-wrap" onSubmit={(event) => { event.preventDefault(); changeFilters({ ...filters, search: search.trim() || undefined }); }}>
      <Select aria-label="日志级别" value={filters.level ?? ""} options={[{ value: "", label: "全部级别" }, ...Object.entries(levelLabels).map(([value, label]) => ({ value, label }))]} onValueChange={(value) => changeFilters({ ...filters, level: value ? value as RuntimeLogLevel : undefined })} className="w-full lg:w-32" />
      <Select aria-label="日志模块" value={filters.module ?? ""} options={[{ value: "", label: "全部模块" }, ...Object.entries(moduleLabels).map(([value, label]) => ({ value, label }))]} onValueChange={(value) => changeFilters({ ...filters, module: value || undefined })} className="w-full lg:w-40" />
      <div className="col-span-2 flex min-w-0 flex-1 gap-2">
        <div className="relative min-w-0 flex-1">
          <Search size={14} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-tertiary" />
          <input aria-label="搜索运行日志" placeholder="搜索事件、原因或账号 ID" maxLength={256} value={search} onChange={(event) => setSearch(event.target.value)} className={controlClass(false, "pl-9")} />
        </div>
        <Button type="submit" variant="secondary">搜索</Button>
        <Button variant="ghost" onClick={() => { setSearch(""); changeFilters({}); }}>重置</Button>
        <Button variant="ghost" aria-label="刷新运行日志" title="刷新运行日志" disabled={query.isFetching} onClick={() => void refresh()}><RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} /><span className="hidden sm:inline">刷新</span></Button>
      </div>
    </form>
    <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 py-3 text-[12px] text-secondary" role="status">
      <span>账号授权、额度同步与系统运行事件</span>
      <span className="flex items-center gap-2"><span className={cn("h-1.5 w-1.5 rounded-full", query.isError ? "bg-warning" : "bg-success")} />{query.isError ? "同步失败，可刷新重试" : query.automatic ? "每 10 秒更新" : "正在浏览历史"}{query.items.length ? ` · 已加载 ${query.items.length} 条` : ""}</span>
    </div>
    {query.items.length > 0 ? <div className="min-w-0 md:flex md:min-h-0 md:flex-1 md:flex-col">
      {mobile ? <>
        <IntersectionSentinel onVisibilityChange={setFollowingLatest} />
        <WindowVirtualList items={query.items} getItemKey={(entry) => entry.id} estimateItemHeight={148} ariaLabel="运行日志列表" renderItem={(entry) => <RuntimeLogRow entry={entry} selected={selected?.id === entry.id} onSelect={setSelected} />} />
        {query.hasNextPage ? <Button className="mt-3 w-full" disabled={query.isFetching} onClick={loadMore}>加载更早日志</Button> : null}
      </> : <AnchoredVirtualRows itemIds={query.items.map((entry) => entry.id)} rowHeight={88} followingLatest={followingLatest} hasMore={query.hasNextPage} loadingMore={query.isFetchingNextPage} historyLoaderKey="runtime-log-history" initialWidth={1000} ariaLabel="运行日志列表" onFollowingLatestChange={setFollowingLatest} onLoadMore={loadMore} renderRow={(index) => <RuntimeLogRow entry={query.items[index]!} selected={selected?.id === query.items[index]!.id} onSelect={setSelected} />} renderHistoryLoader={(loading) => <div className="grid h-11 place-items-center"><Button variant="ghost" disabled={loading} onClick={loadMore}>{loading ? "正在加载更早日志" : "加载更早日志"}</Button></div>} />}
    </div> : <div className="flex min-h-64 flex-1 flex-col items-center justify-center gap-3 px-4 text-center">
      <ScrollText size={24} className="text-tertiary" />
      <p className="text-[13px] font-medium">{query.isPending ? "正在读取运行日志" : query.isError ? "运行日志读取失败" : "暂无符合条件的运行日志"}</p>
      <p className="max-w-md text-[12px] leading-5 text-secondary">{query.isError ? "请刷新重试。" : "启动、账号刷新和后台任务产生的事件会显示在这里。"}</p>
      {query.hasNextPage ? <Button disabled={query.isFetching} onClick={loadMore}>继续查找更早日志</Button> : null}
    </div>}
    <ScrollToTopButton visible={!followingLatest} onClick={() => void refresh()} />
    <RuntimeLogDetailDrawer entry={selected} onClose={() => setSelected(null)} />
  </div>;
}

function RuntimeLogRow({ entry, selected, onSelect }: { entry: RuntimeLogEntry; selected: boolean; onSelect: (entry: RuntimeLogEntry) => void }) {
  const reason = eventReason(entry);
  const account = entry.fields.oauth_account_name ?? entry.fields.oauth_account_id ?? entry.fields.provider;
  return <button type="button" aria-label={`查看事件：${eventTitle(entry)}`} onClick={() => onSelect(entry)} className={cn("focus-ring my-1 flex w-full min-w-0 flex-col gap-2 rounded-xl border border-subtle bg-surface px-3 py-3 text-left transition-colors hover:bg-surface-muted/60 md:my-0 md:h-[88px] md:rounded-none md:border-x-0 md:border-t-0 md:px-3", selected && "bg-accent/5")}>
    <div className="flex w-full min-w-0 items-center gap-2">
      <RuntimeLogLevelBadge level={entry.level} />
      <span className="min-w-0 flex-1 truncate text-[12px] text-secondary">{moduleLabels[entry.module] ?? entry.module}</span>
      <time dateTime={entry.timestamp} className="shrink-0 text-[12px] tabular-nums text-secondary">{formatCompactDateTime(Date.parse(entry.timestamp))}</time>
      <ChevronRight size={14} className="shrink-0 text-tertiary" />
    </div>
    <div className="flex w-full min-w-0 flex-col gap-1 md:flex-row md:items-center md:gap-4">
      <p className="min-w-0 flex-1 break-words text-[13px] font-medium leading-5 [overflow-wrap:anywhere] md:truncate">{eventTitle(entry)}{reason ? <span className="ml-2 font-normal text-secondary">{reason}</span> : null}</p>
      {account ? <span className="max-w-full truncate text-[12px] text-secondary md:max-w-48" title={account}>{account}</span> : null}
    </div>
  </button>;
}
