import { Activity, Plus, RefreshCw, Search } from "lucide-react";
import { useMemo, useState } from "react";

import type { ProviderEndpoint } from "@/features/providers";
import type { ProxyConfiguration, ProxyProfile } from "../api/proxy-contracts";
import type { ProxyTestResult } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { isCurrentTestResult } from "./proxy-test-result";
import { ProxyTableRow } from "./ProxyTableRow";
import { Button } from "@/shared/ui/Button";

interface ProxyListProps {
  configuration: ProxyConfiguration;
  pending: boolean;
  refreshing: boolean;
  actionError: unknown;
  endpoints: ProviderEndpoint[];
  endpointsLoading: boolean;
  endpointError: unknown;
  testEndpointId: string;
  testingProxyId: string | null;
  testResults: Record<string, ProxyTestResult>;
  testError: unknown;
  testErrorProxyId: string | null;
  onCreate: () => void;
  onRefresh: () => void;
  onTestEndpointChange: (id: string) => void;
  onTest: (id: string) => void;
  onEdit: (id: string) => void;
  onSetGlobal: (proxy: ProxyProfile) => void;
  onDelete: (proxy: ProxyProfile) => void;
}

export function ProxyList({
  configuration,
  pending,
  refreshing,
  actionError,
  endpoints,
  endpointsLoading,
  endpointError,
  testEndpointId,
  testingProxyId,
  testResults,
  testError,
  testErrorProxyId,
  onCreate,
  onRefresh,
  onTestEndpointChange,
  onTest,
  onEdit,
  onSetGlobal,
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
        <label className="relative min-w-0 sm:w-52">
          <span className="sr-only">搜索出口代理</span>
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

        <div className="flex flex-wrap items-center gap-1.5 sm:justify-end">
          <label className="relative min-w-0 sm:w-52">
            <span className="sr-only">代理测试目标</span>
            <Activity
              size={13}
              className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-tertiary"
              aria-hidden="true"
            />
            <select
              aria-label="代理测试目标"
              className="field-select focus-ring h-8 w-full min-w-0 cursor-pointer appearance-none rounded-[8px] border-0 bg-surface-muted py-0 pl-8 pr-8 text-[12px] text-primary disabled:cursor-default"
              value={testEndpointId}
              disabled={endpointsLoading || endpoints.length === 0 || testingProxyId !== null}
              onChange={(event) => onTestEndpointChange(event.target.value)}
            >
              {endpointsLoading ? <option value="">正在读取测试目标</option> : null}
              {!endpointsLoading && endpointError && endpoints.length === 0 ? (
                <option value="">测试目标加载失败</option>
              ) : null}
              {!endpointsLoading && !endpointError && endpoints.length === 0 ? (
                <option value="">暂无 Provider Endpoint</option>
              ) : null}
              {endpoints.map((endpoint) => (
                <option key={endpoint.id} value={endpoint.id}>
                  {endpoint.name}{!endpoint.enabled ? "（已停用）" : ""}
                </option>
              ))}
            </select>
          </label>
          <Button variant="ghost" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw size={14} className={refreshing ? "animate-spin" : undefined} />
            刷新
          </Button>
          <Button variant="primary" onClick={onCreate} disabled={pending}>
            <Plus size={14} />
            新增
          </Button>
        </div>
      </div>

      {endpointError ? (
        <p className="border-b border-subtle py-2.5 text-[12px] text-danger" role="alert">
          测试目标加载失败：{getProxyErrorMessage(endpointError)}
        </p>
      ) : !endpointsLoading && endpoints.length === 0 ? (
        <p className="border-b border-subtle py-2.5 text-[12px] text-secondary" role="status">
          暂无 Provider Endpoint，代理连通性测试不可用。
        </p>
      ) : null}

      <div className="overflow-x-auto">
        <table className="w-full min-w-[840px] border-collapse text-left text-[12px]">
          <caption className="sr-only">出口代理列表</caption>
          <thead>
            <tr className="border-b border-subtle text-secondary">
              <th className="py-2.5 pr-3 font-medium">名称</th>
              <th className="px-3 py-2.5 font-medium">类型</th>
              <th className="px-3 py-2.5 font-medium">地址</th>
              <th className="px-3 py-2.5 font-medium">状态</th>
              <th className="px-3 py-2.5 font-medium">认证</th>
              <th className="px-3 py-2.5 font-medium">连通性</th>
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
                canTest={testEndpointId.length > 0}
                testing={testingProxyId === proxy.id}
                testPending={testingProxyId !== null}
                testResult={
                  isCurrentTestResult(
                    testResults[proxy.id],
                    proxy,
                    configuration.configRevision,
                    endpoints,
                    testEndpointId,
                  )
                    ? testResults[proxy.id]
                    : undefined
                }
                testError={testErrorProxyId === proxy.id ? testError : null}
                onTest={() => onTest(proxy.id)}
                onEdit={onEdit}
                onSetGlobal={onSetGlobal}
                onDelete={onDelete}
              />
            ))}
          </tbody>
        </table>
      </div>

      {filtered.length === 0 ? (
        <p className="py-8 text-center text-sm text-secondary">
          {query.trim() ? "没有匹配的出口代理" : "暂无出口代理"}
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
