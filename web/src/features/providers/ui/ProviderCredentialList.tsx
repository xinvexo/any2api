import { Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type {
  ProviderCredential,
  ProviderCredentialConfiguration,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { ProviderCredentialTableRow } from "./ProviderCredentialTableRow";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";

interface ProviderCredentialListProps {
  configuration: ProviderCredentialConfiguration;
  proxies: ProxyConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  endpoint: ProviderEndpoint;
  testingCredentialId: string | null;
  testResults: Record<string, ProviderCredentialTestResult>;
  testError: unknown;
  onCreate: () => void;
  onRefresh: () => void;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
  onTest: (id: string) => void;
}

export function ProviderCredentialList({
  configuration,
  proxies,
  pending,
  refreshing,
  actionError,
  endpoint,
  testingCredentialId,
  testResults,
  testError,
  onCreate,
  onRefresh,
  onEdit,
  onRotate,
  onDelete,
  onTest,
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
            placeholder="搜索名称或代理"
            aria-label="搜索 API Key"
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
            {configuration.items.length === 0 ? "还没有 API Key" : "没有匹配的 API Key"}
          </p>
          <p className="mt-1 text-[12px] text-secondary">
            {configuration.items.length === 0
              ? "为这个 Endpoint 添加上游凭据。"
              : "试试其他关键词。"}
          </p>
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full min-w-[760px] border-collapse text-left text-[12px]">
            <thead>
              <tr className="border-b border-subtle text-[11px] font-medium uppercase tracking-wide text-tertiary">
                <th className="py-2.5 pr-3 font-medium">名称</th>
                <th className="px-3 py-2.5 font-medium">代理</th>
                <th className="px-3 py-2.5 font-medium">并发</th>
                <th className="px-3 py-2.5 font-medium">状态</th>
                <th className="px-3 py-2.5 font-medium">密钥</th>
                <th className="py-2.5 pl-3 text-right font-medium">操作</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((credential) => (
                <ProviderCredentialTableRow
                  key={credential.id}
                  credential={credential}
                  proxies={proxies}
                  endpoint={endpoint}
                  configRevision={configuration.configRevision}
                  pending={pending}
                  testing={testingCredentialId === credential.id}
                  testResult={testResults[credential.id]}
                  onEdit={onEdit}
                  onRotate={onRotate}
                  onDelete={onDelete}
                  onTest={onTest}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2 border-t border-subtle py-3 text-[12px] text-secondary">
        <p>
          API Key · 配置版本{" "}
          <span className="font-medium tabular-nums text-primary">{configuration.configRevision}</span>
          {" · "}
          共 <span className="tabular-nums">{filtered.length}</span> 条
        </p>
      </div>

      {testError ? (
        <p className="border-t border-subtle py-3 text-sm text-danger" role="alert">
          {getProviderErrorMessage(testError)}
        </p>
      ) : null}

      {actionError ? (
        <p className="border-t border-subtle py-3 text-sm text-danger" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
    </div>
  );
}
