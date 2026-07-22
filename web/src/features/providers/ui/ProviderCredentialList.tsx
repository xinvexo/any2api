import { Activity, Check, KeyRound, Pencil, RotateCw, Trash2, X } from "lucide-react";
import { useState } from "react";

import type {
  ProviderCredential,
  ProviderCredentialConfiguration,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import type { ProviderEndpoint } from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProviderCredentialListProps {
  configuration: ProviderCredentialConfiguration;
  proxies: ProxyConfiguration;
  pending: boolean;
  actionError: unknown;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
  endpoint: ProviderEndpoint;
  testingCredentialId: string | null;
  testResults: Record<string, ProviderCredentialTestResult>;
  testError: unknown;
  onTest: (id: string) => void;
}

export function ProviderCredentialList({
  configuration,
  proxies,
  pending,
  actionError,
  onEdit,
  onRotate,
  onDelete,
  endpoint,
  testingCredentialId,
  testResults,
  testError,
  onTest,
}: ProviderCredentialListProps) {
  return (
    <Surface className="overflow-hidden">
      <div className="flex items-center justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div>
          <h2 className="text-base font-semibold">API Key</h2>
          <p className="mt-1 text-sm text-secondary">独立代理、并发上限与启停状态</p>
        </div>
        <span className="text-sm tabular-nums text-tertiary">{configuration.items.length}</span>
      </div>
      {configuration.items.length > 0 ? (
        <ul className="divide-y divide-subtle">
          {configuration.items.map((credential) => (
            <CredentialRow
              key={credential.id}
              credential={credential}
              proxies={proxies}
              pending={pending}
              onEdit={onEdit}
              onRotate={onRotate}
              onDelete={onDelete}
              endpoint={endpoint}
              testing={testingCredentialId === credential.id}
              testPending={testingCredentialId !== null}
              testResult={currentTestResult(
                testResults[credential.id],
                credential,
                configuration.configRevision,
                endpoint,
                proxies,
              )}
              onTest={() => onTest(credential.id)}
            />
          ))}
        </ul>
      ) : (
        <div className="p-7 text-center">
          <KeyRound size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 text-sm font-medium">还没有 API Key</p>
          <p className="mt-1 text-sm text-secondary">添加后即可为这个 Endpoint 提供独立凭据。</p>
        </div>
      )}
      {actionError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProviderErrorMessage(actionError)}
        </p>
      ) : null}
      {testError ? (
        <p className="border-t border-subtle px-5 py-3 text-sm text-danger sm:px-6" role="alert">
          {getProviderErrorMessage(testError)}
        </p>
      ) : null}
    </Surface>
  );
}

function CredentialRow({
  credential,
  proxies,
  pending,
  onEdit,
  onRotate,
  onDelete,
  endpoint,
  testing,
  testPending,
  testResult,
  onTest,
}: {
  credential: ProviderCredential;
  proxies: ProxyConfiguration;
  pending: boolean;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
  endpoint: ProviderEndpoint;
  testing: boolean;
  testPending: boolean;
  testResult?: ProviderCredentialTestResult;
  onTest: () => void;
}) {
  const [confirmDelete, setConfirmDelete] = useState(false);
  const proxy = proxies.items.find((item) => item.id === credential.proxyProfileId);
  const actualProxy = resolveProxy(credential, proxies);
  const proxyLabel = describeProxy(proxy?.id, proxies);
  const canTest = credential.enabled && endpoint.enabled && actualProxy?.enabled === true;

  return (
    <li className="px-5 py-5 sm:px-6">
      <div className="flex flex-col gap-4 xl:flex-row xl:items-center xl:justify-between">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <p className="break-words font-semibold [overflow-wrap:anywhere]">{credential.label}</p>
            <Badge icon={credential.enabled ? <Check size={12} /> : <X size={12} />}>
              {credential.enabled ? "启用" : "已停用"}
            </Badge>
            <Badge>并发 {credential.maxConcurrency}</Badge>
          </div>
          <p className="mt-2 break-words text-sm text-secondary [overflow-wrap:anywhere]">
            {proxyLabel}
          </p>
          <p className="mt-1 font-mono text-xs text-tertiary">
            {credential.secretTail ? `•••• ${credential.secretTail}` : credential.fingerprint}
          </p>
          {testResult ? <CredentialTestResult result={testResult} /> : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            variant="ghost"
            disabled={pending || testPending || !canTest}
            aria-label={`测试 ${credential.label}`}
            onClick={onTest}
          >
            <Activity size={15} className={testing ? "animate-pulse" : undefined} />
            {testing ? "测试中" : "测试"}
          </Button>
          <Button
            variant="ghost"
            disabled={pending}
            aria-label={`编辑 ${credential.label}`}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={15} />
            编辑
          </Button>
          <Button
            variant="ghost"
            disabled={pending}
            aria-label={`轮换 ${credential.label}`}
            onClick={() => onRotate(credential.id)}
          >
            <RotateCw size={15} />
            轮换
          </Button>
          {confirmDelete ? (
            <>
              <Button
                variant="danger"
                disabled={pending}
                aria-label={`确认删除 ${credential.label}`}
                onClick={() => onDelete(credential)}
              >
                <Trash2 size={15} />
                确认删除
              </Button>
              <Button
                variant="ghost"
                disabled={pending}
                aria-label={`取消删除 ${credential.label}`}
                onClick={() => setConfirmDelete(false)}
              >
                <X size={15} />
                取消
              </Button>
            </>
          ) : (
            <Button
              variant="ghost"
              disabled={pending}
              aria-label={`删除 ${credential.label}`}
              onClick={() => setConfirmDelete(true)}
            >
              <Trash2 size={15} />
              删除
            </Button>
          )}
        </div>
      </div>
    </li>
  );
}

function currentTestResult(
  result: ProviderCredentialTestResult | undefined,
  credential: ProviderCredential,
  configRevision: number,
  endpoint: ProviderEndpoint,
  proxies: ProxyConfiguration,
) {
  if (!result) {
    return undefined;
  }
  const proxy = resolveProxy(credential, proxies);
  return result.configRevision === configRevision &&
    result.providerEndpointId === endpoint.id &&
    result.providerEndpointConfigVersion === endpoint.configVersion &&
    result.credentialId === credential.id &&
    result.credentialConfigVersion === credential.configVersion &&
    result.credentialGeneration === credential.credentialGeneration &&
    result.secretVersion === credential.secretVersion &&
    result.proxyId === proxy?.id &&
    result.proxyConfigVersion === proxy.configVersion
    ? result
    : undefined;
}

function resolveProxy(credential: ProviderCredential, configuration: ProxyConfiguration) {
  const bound = configuration.items.find((item) => item.id === credential.proxyProfileId);
  if (bound?.kind !== "direct") {
    return bound;
  }
  return configuration.items.find((item) => item.id === configuration.globalProxyId);
}

function CredentialTestResult({ result }: { result: ProviderCredentialTestResult }) {
  if (result.accepted) {
    return (
      <p className="mt-1 text-xs text-success">
        可用 · HTTP {result.statusCode} · {result.latencyMs} ms
        {result.authErrorCleared ? " · 已清除认证错误" : ""}
      </p>
    );
  }
  if (result.reachable) {
    return (
      <p className="mt-1 text-xs text-warning">
        已连接 · HTTP {result.statusCode} · 凭据未通过
      </p>
    );
  }
  return (
    <p className="mt-1 text-xs text-danger">
      失败 · {result.errorStage ?? "unknown"} · {result.failureScope ?? "unknown"}
    </p>
  );
}

function describeProxy(proxyId: string | undefined, configuration: ProxyConfiguration) {
  const proxy = configuration.items.find((item) => item.id === proxyId);
  if (!proxy) {
    return "代理配置不存在";
  }
  if (proxy.kind !== "direct") {
    return `${proxy.name} · ${proxy.kind.toUpperCase()}${proxy.enabled ? "" : " · 已停用"}`;
  }
  const global = configuration.items.find((item) => item.id === configuration.globalProxyId);
  return global?.kind === "direct"
    ? "DIRECT · 本机直连"
    : `DIRECT · 继承全局 ${global?.name ?? "未知代理"}`;
}

function Badge({ children, icon }: { children: React.ReactNode; icon?: React.ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-surface-muted px-2 py-1 text-[11px] font-semibold text-secondary">
      {icon}
      {children}
    </span>
  );
}
