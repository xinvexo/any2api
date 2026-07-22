import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type {
  ProviderEndpoint,
  ProviderEndpointConfiguration,
} from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { ProviderEndpointTableRow } from "./ProviderEndpointTableRow";
import { Button } from "@/shared/ui/Button";

interface ProviderEndpointListProps {
  configuration: ProviderEndpointConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  onCreate: () => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onDelete: (endpoint: ProviderEndpoint) => void;
}

export function ProviderEndpointList({
  configuration,
  pending,
  refreshing,
  actionError,
  onCreate,
  onRefresh,
  onEdit,
  onDelete,
}: ProviderEndpointListProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return configuration.items;
    }
    return configuration.items.filter((endpoint) =>
      [endpoint.name, endpoint.providerKind, endpoint.baseUrl, endpoint.protocolDialect]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    );
  }, [configuration.items, query]);

  return (
    <div>
      <div className="flex flex-col gap-2.5 border-b border-subtle pb-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="relative min-w-0 flex-1 sm:max-w-sm">
          <Search
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
            aria-hidden="true"
          />
          <input
            className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
            value={query}
            placeholder="搜索名称、类型或 URL"
            aria-label="搜索 Provider"
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button variant="ghost" disabled={refreshing} onClick={onRefresh}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" disabled={pending} onClick={onCreate}>
            <Plus size={14} />
            新增
          </Button>
        </div>
      </div>

      {filtered.length === 0 ? (
        <div className="flex min-h-48 flex-col items-center justify-center px-4 py-10 text-center">
          <p className="text-[13px] font-medium">
            {configuration.items.length === 0 ? "还没有 Provider Endpoint" : "没有匹配的 Endpoint"}
          </p>
          <p className="mt-1 text-[12px] text-secondary">
            {configuration.items.length === 0
              ? "添加 Codex 或 Claude 上游地址。"
              : "试试其他关键词。"}
          </p>
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[720px] border-collapse text-left text-[12px]">
            <thead>
              <tr className="border-b border-subtle text-[11px] font-medium uppercase tracking-wide text-tertiary">
                <th className="py-2.5 pr-3 font-medium">名称</th>
                <th className="px-3 py-2.5 font-medium">类型</th>
                <th className="px-3 py-2.5 font-medium">Base URL</th>
                <th className="px-3 py-2.5 font-medium">状态</th>
                <th className="py-2.5 pl-3 text-right font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((endpoint) => (
                <ProviderEndpointTableRow
                  key={endpoint.id}
                  endpoint={endpoint}
                  pending={pending}
                  onEdit={onEdit}
                  onDelete={onDelete}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
        <p>
          Provider · 配置版本{" "}
          <span className="font-medium tabular-nums text-primary">{configuration.configRevision}</span>
          {" · "}
          共 <span className="tabular-nums">{filtered.length}</span> 条
        </p>
      </div>

      {actionError ? (
        <p className="border-t border-subtle py-3 text-sm text-danger" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
    </div>
  );
}
