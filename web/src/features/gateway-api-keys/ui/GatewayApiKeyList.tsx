import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type { GatewayApiKey, GatewayApiKeyConfiguration } from "../api/gateway-api-key-contracts";
import { getGatewayApiKeyErrorMessage } from "../model/gateway-api-key-error";
import { GatewayApiKeyTableRow } from "./GatewayApiKeyTableRow";
import { Button } from "@/shared/ui/Button";

interface GatewayApiKeyListProps {
  configuration: GatewayApiKeyConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  onCreate: () => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onDelete: (key: GatewayApiKey) => void;
}

export function GatewayApiKeyList({
  configuration,
  pending,
  refreshing,
  actionError,
  onCreate,
  onRefresh,
  onEdit,
  onDelete,
}: GatewayApiKeyListProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return configuration.items;
    }
    return configuration.items.filter((key) => {
      const status = key.enabled ? "已启用" : "已停用";
      return [key.name, status, key.token].join(" ").toLowerCase().includes(needle);
    });
  }, [configuration.items, query]);

  return (
    <div>
      <div className="flex flex-col gap-2.5 border-b border-subtle pb-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex flex-wrap items-center gap-1.5">
          <Button variant="primary" onClick={onCreate} disabled={pending}>
            <Plus size={14} />
            新增密钥
          </Button>
          <Button variant="ghost" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>

        <label className="relative min-w-0 sm:w-52">
          <span className="sr-only">搜索密钥</span>
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
        <table className="w-full min-w-[760px] border-collapse text-left text-[12px]">
          <caption className="sr-only">网关密钥列表</caption>
          <thead>
            <tr className="border-b border-subtle text-secondary">
              <th className="py-2.5 pr-3 font-medium">名称</th>
              <th className="px-3 py-2.5 font-medium">密钥</th>
              <th className="px-3 py-2.5 font-medium">状态</th>
              <th className="px-3 py-2.5 font-medium">最后使用</th>
              <th className="px-3 py-2.5 font-medium">创建时间</th>
              <th className="py-2.5 pl-3 text-right font-medium">操作</th>
            </tr>
          </thead>
          <tbody>
            {filtered.map((key) => (
              <GatewayApiKeyTableRow
                key={key.id}
                apiKey={key}
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
          {query.trim()
            ? "没有匹配的密钥"
            : "尚未创建网关密钥。客户端使用这些密钥访问本地网关。"}
        </p>
      ) : null}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
        <p>
          共 <span className="tabular-nums">{filtered.length}</span> 条
        </p>
      </div>

      {actionError ? (
        <p className="border-t border-subtle py-3 text-sm text-danger" role="alert">
          {getGatewayApiKeyErrorMessage(actionError)}
        </p>
      ) : null}
    </div>
  );
}
