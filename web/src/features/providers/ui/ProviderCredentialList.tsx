import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type {
  ProviderCredential,
  ProviderCredentialConfiguration,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { ProviderCredentialTableRow } from "./ProviderCredentialTableRow";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { cn } from "@/shared/lib/cn";

export interface ProviderCredentialListProps {
  configuration: ProviderCredentialConfiguration;
  proxies: ProxyConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  embedded?: boolean;
  onCreate: () => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onModels: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
}

export function ProviderCredentialList({
  configuration,
  proxies,
  pending,
  refreshing,
  actionError,
  embedded = false,
  onCreate,
  onRefresh,
  onEdit,
  onModels,
  onDelete,
}: ProviderCredentialListProps) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return configuration.items;
    }
    return configuration.items.filter((credential) => {
      const proxy = proxies.items.find((item) => item.id === credential.proxyProfileId);
      return [credential.label, proxy?.name ?? "", credential.secretTail ?? "", credential.fingerprint]
        .join(" ")
        .toLowerCase()
        .includes(needle);
    });
  }, [configuration.items, proxies.items, query]);

  return (
    <div>
      {!embedded ? (
        <div className="flex items-center justify-between gap-2 border-b border-subtle pb-3">
          <div className="relative min-w-0 flex-1 sm:max-w-sm">
            <Search
              size={14}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
              aria-hidden="true"
            />
            <input
              className="focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-3 text-[12px] text-primary placeholder:text-tertiary"
              value={query}
              placeholder="搜索名称或出口代理"
              aria-label="搜索 API Key"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          <div className="flex shrink-0 items-center gap-1">
            <Button variant="ghost" disabled={refreshing} onClick={onRefresh}>
              <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
              刷新
            </Button>
            <Button variant="ghost" disabled={pending} onClick={onCreate}>
              <Plus size={14} />
              新增
            </Button>
          </div>
        </div>
      ) : null}

      {filtered.length === 0 ? (
        <p
          className={cn(
            "text-[12px] text-tertiary",
            embedded ? "py-2.5" : "min-h-48 px-4 py-10 text-center text-[13px] text-secondary",
          )}
        >
          {configuration.items.length === 0
            ? embedded
              ? "暂无 API Key"
              : "还没有 API Key"
            : "没有匹配的 API Key"}
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table
            className={cn(
              "w-full border-collapse text-left",
              embedded ? "min-w-[800px] text-[11px]" : "min-w-[980px] text-[12px]",
            )}
          >
            <thead>
              <tr
                className={cn(
                  "text-tertiary",
                  embedded ? "text-[10px] font-normal" : "text-[11px]",
                )}
              >
                <th className="py-1.5 pr-3 font-medium">名称</th>
                <th className="px-3 py-1.5 font-medium">出口代理</th>
                <th className="px-3 py-1.5 font-medium">并发</th>
                <th className="px-3 py-1.5 font-medium">状态</th>
                <th className="px-3 py-1.5 font-medium">密钥</th>
                <th className="px-3 py-1.5 font-medium">请求统计</th>
                <th className="py-1.5 pl-3 text-right font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((credential) => (
                <ProviderCredentialTableRow
                  key={credential.id}
                  credential={credential}
                  proxies={proxies}
                  pending={pending}
                  embedded={embedded}
                  onEdit={onEdit}
                  onModels={onModels}
                  onDelete={onDelete}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      {!embedded ? (
        <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
          <p>
            共 <span className="tabular-nums">{filtered.length}</span> 条
          </p>
        </div>
      ) : null}

      {actionError ? (
        <p className="py-2 text-[12px] text-danger" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
    </div>
  );
}
