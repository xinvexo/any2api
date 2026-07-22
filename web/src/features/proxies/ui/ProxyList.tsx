import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type { ProxyConfiguration } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { ProxyTableRow } from "./ProxyTableRow";
import { Button } from "@/shared/ui/Button";

interface ProxyListProps {
  configuration: ProxyConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  onCreate: () => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
}

export function ProxyList({
  configuration,
  pending,
  refreshing,
  actionError,
  onCreate,
  onRefresh,
  onEdit,
  onDelete,
}: ProxyListProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return configuration.items;
    }
    return configuration.items.filter((proxy) => {
      const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";
      return [proxy.name, proxy.kind, endpoint, proxy.username ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  }, [configuration.items, query]);

  return (
    <div>
      <div className="flex flex-col gap-2.5 border-b border-subtle pb-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
          <Button variant="ghost" onClick={onCreate} disabled={pending}>
            <Plus size={14} />
            新增代理
          </Button>
          <Button variant="ghost" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>

        <label className="relative min-w-0 sm:w-52">
          <span className="sr-only">搜索代理</span>
          <Search
            size={13}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
            aria-hidden="true"
          />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索关键字"
            className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
          />
        </label>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full min-w-[680px] border-collapse text-left text-[12px]">
          <caption className="sr-only">代理列表</caption>
          <thead>
            <tr className="border-b border-subtle text-secondary">
              <th className="py-2.5 pr-3 font-medium">名称</th>
              <th className="px-3 py-2.5 font-medium">类型</th>
              <th className="px-3 py-2.5 font-medium">地址</th>
              <th className="px-3 py-2.5 font-medium">状态</th>
              <th className="px-3 py-2.5 font-medium">认证</th>
              <th className="py-2.5 pl-3 text-right font-medium">操作</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((proxy) => (
              <ProxyTableRow
                key={proxy.id}
                proxy={proxy}
                isGlobal={proxy.id === configuration.globalProxyId}
                pending={pending}
                onEdit={onEdit}
                onDelete={onDelete}
              />
            ))}
          </tbody>
        </table>
      </div>

      {filtered.length === 0 ? (
        <p className="border-t border-subtle py-8 text-center text-sm text-secondary">
          {query.trim() ? "没有匹配的代理" : "暂无代理"}
        </p>
      ) : null}

      {configuration.items.length === 1 && !query.trim() ? (
        <p className="border-t border-subtle py-3 text-sm text-secondary">
          尚未添加自定义代理。新代理会独立保存，不会改变当前全局出口。
        </p>
      ) : null}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
        <p>
          共 <span className="tabular-nums">{filtered.length}</span> 条
        </p>
      </div>

      {actionError ? (
        <p className="border-t border-subtle py-3 text-sm text-danger" role="alert">
          {getProxyErrorMessage(actionError)}
        </p>
      ) : null}
    </div>
  );
}
