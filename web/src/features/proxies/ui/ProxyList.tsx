import { Activity, Check, Pencil, ShieldCheck, Trash2, X } from "lucide-react";
import { useState } from "react";

import type { ProviderEndpoint } from "@/features/providers";
import type { ProxyConfiguration, ProxyProfile } from "../api/proxy-contracts";
import type { ProxyTestResult } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProxyListProps {
  configuration: ProxyConfiguration;
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onSetGlobal: (id: string) => void;
  onDelete: (id: string) => void;
  endpoints: ProviderEndpoint[];
  testEndpointId: string;
  testingProxyId: string | null;
  testResults: Record<string, ProxyTestResult>;
  onTestEndpointChange: (id: string) => void;
  onTest: (id: string) => void;
  testError: unknown;
}

export function ProxyList({
  configuration,
  pending,
  actionError,
  onEdit,
  onSetGlobal,
  onDelete,
  endpoints,
  testEndpointId,
  testingProxyId,
  testResults,
  onTestEndpointChange,
  onTest,
  testError,
}: ProxyListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">代理列表</h2>
          <p className="mt-1 text-sm text-secondary">DIRECT 固定置顶且始终可用</p>
        </div>
        <div className="flex items-center gap-3">
          <select
            aria-label="代理测试目标"
            className="focus-ring h-9 max-w-52 rounded-control border border-subtle bg-surface px-2 text-xs text-secondary"
            value={testEndpointId}
            onChange={(event) => onTestEndpointChange(event.target.value)}
          >
            <option value="">选择测试目标</option>
            {endpoints.map((endpoint) => (
              <option key={endpoint.id} value={endpoint.id}>
                {endpoint.name}
                {!endpoint.enabled ? "（已停用）" : ""}
              </option>
            ))}
          </select>
          <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
        </div>
      </div>
      <ul className="divide-y divide-subtle">
        {configuration.items.map((proxy) => (
          <ProxyRow
            key={proxy.id}
            proxy={proxy}
            isGlobal={proxy.id === configuration.globalProxyId}
            pending={pending}
            onEdit={onEdit}
            onSetGlobal={onSetGlobal}
            onDelete={onDelete}
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
            onTest={() => onTest(proxy.id)}
          />
        ))}
      </ul>
      {configuration.items.length === 1 ? (
        <p className="border-t border-subtle px-5 py-4 text-sm text-secondary sm:px-6">
          尚未添加自定义代理。新代理会独立保存，不会改变当前全局出口。
        </p>
      ) : null}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProxyErrorMessage(actionError)}
        </p>
      ) : null}
      {testError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProxyErrorMessage(testError)}
        </p>
      ) : null}
    </Surface>
  );
}

interface ProxyRowProps {
  proxy: ProxyProfile;
  isGlobal: boolean;
  pending: boolean;
  onEdit: (id: string) => void;
  onSetGlobal: (id: string) => void;
  onDelete: (id: string) => void;
  canTest: boolean;
  testing: boolean;
  testPending: boolean;
  testResult?: ProxyTestResult;
  onTest: () => void;
}

function ProxyRow({
  proxy,
  isGlobal,
  pending,
  onEdit,
  onSetGlobal,
  onDelete,
  canTest,
  testing,
  testPending,
  testResult,
  onTest,
}: ProxyRowProps) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const endpoint = proxy.host && proxy.port ? `${proxy.host}:${proxy.port}` : "本机网络";

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="min-w-0 break-words font-semibold [overflow-wrap:anywhere]">{proxy.name}</p>
            <Badge>{proxy.kind.toUpperCase()}</Badge>
            {proxy.builtIn ? <Badge icon={<ShieldCheck size={12} />}>内置</Badge> : null}
            {isGlobal ? <Badge icon={<Check size={12} />}>全局</Badge> : null}
            {!proxy.enabled ? <Badge>已停用</Badge> : null}
          </div>
          <p className="mt-2 break-all text-sm text-secondary">{endpoint}</p>
          {testResult ? <TestResult result={testResult} /> : null}
          {proxy.builtIn ? (
            <p className="mt-1 text-xs text-tertiary">不可编辑、删除或停用</p>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="ghost"
            disabled={!canTest || !proxy.enabled || testPending || pending}
            aria-label={`测试 ${proxy.name}`}
            onClick={onTest}
          >
            <Activity size={15} className={testing ? "animate-pulse" : undefined} />
            {testing ? "测试中" : "测试"}
          </Button>
          {!proxy.builtIn ? (
            <>
            {!isGlobal && proxy.enabled ? (
              <Button
                variant="ghost"
                disabled={pending}
                aria-label={`设为全局 ${proxy.name}`}
                onClick={() => onSetGlobal(proxy.id)}
              >
                设为全局
              </Button>
            ) : null}
            <Button
              variant="ghost"
              disabled={pending}
              aria-label={`编辑 ${proxy.name}`}
              onClick={() => onEdit(proxy.id)}
            >
              <Pencil size={15} />
              编辑
            </Button>
            {confirmDelete ? (
              <>
                <Button
                  variant="danger"
                  disabled={pending || isGlobal}
                  aria-label={`确认删除 ${proxy.name}`}
                  onClick={() => onDelete(proxy.id)}
                >
                  <Trash2 size={15} />
                  确认删除
                </Button>
                <Button
                  variant="ghost"
                  disabled={pending}
                  aria-label={`取消删除 ${proxy.name}`}
                  onClick={() => setConfirmDelete(false)}
                >
                  <X size={15} />
                  取消
                </Button>
              </>
            ) : (
              <Button
                variant="ghost"
                disabled={pending || isGlobal}
                aria-label={`删除 ${proxy.name}`}
                onClick={() => setConfirmDelete(true)}
              >
                <Trash2 size={15} />
                删除
              </Button>
            )}
            </>
          ) : null}
        </div>
      </div>
    </li>
  );
}

function isCurrentTestResult(
  result: ProxyTestResult | undefined,
  proxy: ProxyProfile,
  configRevision: number,
  endpoints: ProviderEndpoint[],
  selectedEndpointId: string,
) {
  if (!result || result.providerEndpointId !== selectedEndpointId) {
    return false;
  }
  const endpoint = endpoints.find((candidate) => candidate.id === result.providerEndpointId);
  return (
    result.configRevision === configRevision &&
    result.proxyConfigVersion === proxy.configVersion &&
    endpoint?.configVersion === result.providerEndpointConfigVersion
  );
}

function TestResult({ result }: { result: ProxyTestResult }) {
  return result.reachable ? (
    <p className="mt-1 text-xs text-success">
      可达 · HTTP {result.statusCode} · {result.latencyMs} ms
    </p>
  ) : (
    <p className="mt-1 text-xs text-danger">
      失败 · {result.errorStage ?? "unknown"} · {result.failureScope ?? "unknown"}
    </p>
  );
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
