import { Activity, Pencil, RotateCw, Trash2 } from "lucide-react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import type { ProxyConfiguration } from "@/features/proxies";
import { cn } from "@/shared/lib/cn";

export interface ProviderCredentialTableRowProps {
  credential: ProviderCredential;
  proxies: ProxyConfiguration;
  endpoint: ProviderEndpoint;
  configRevision: number;
  pending: boolean;
  testing: boolean;
  testResult?: ProviderCredentialTestResult;
  onEdit: (id: string) => void;
  onRotate: (id: string) => void;
  onDelete: (credential: ProviderCredential) => void;
  onTest: (id: string) => void;
}

export function ProviderCredentialTableRow({
  credential,
  proxies,
  endpoint,
  configRevision,
  pending,
  testing,
  testResult,
  onEdit,
  onRotate,
  onDelete,
  onTest,
}: ProviderCredentialTableRowProps) {
  const proxyLabel = describeProxy(credential.proxyProfileId, proxies);
  const canTest = Boolean(resolveProxy(credential, proxies));
  const currentResult = currentTestResult(
    testResult,
    credential,
    configRevision,
    endpoint,
    proxies,
  );

  return (
    <tr className="border-b border-subtle last:border-b-0">
      <td className="py-2.5 pr-3 align-middle">
        <p className="break-words font-medium text-primary [overflow-wrap:anywhere]">
          {credential.label}
        </p>
        {currentResult ? <CredentialTestResult result={currentResult} /> : null}
      </td>
      <td className="px-3 py-2.5 align-middle">
        <span className="break-words text-secondary [overflow-wrap:anywhere]">{proxyLabel}</span>
      </td>
      <td className="px-3 py-2.5 align-middle tabular-nums text-secondary">
        {credential.maxConcurrency}
      </td>
      <td className="px-3 py-2.5 align-middle">
        {credential.enabled ? <Badge tone="success">已启用</Badge> : <Badge>已停用</Badge>}
      </td>
      <td className="px-3 py-2.5 align-middle">
        <span className="font-mono text-[11px] text-tertiary">
          {credential.secretTail ? `•••• ${credential.secretTail}` : credential.fingerprint}
        </span>
      </td>
      <td className="py-2.5 pl-3 align-middle">
        <div className="flex flex-wrap items-center justify-end gap-0.5">
          <RowAction
            label={`测试 ${credential.label}`}
            disabled={pending || testing || !canTest}
            onClick={() => onTest(credential.id)}
          >
            <Activity size={13} className={testing ? "animate-pulse" : undefined} />
            {testing ? "测试中" : "测试"}
          </RowAction>
          <RowAction
            label={`编辑 ${credential.label}`}
            disabled={pending}
            onClick={() => onEdit(credential.id)}
          >
            <Pencil size={13} />
            编辑
          </RowAction>
          <RowAction
            label={`轮换 ${credential.label}`}
            disabled={pending}
            onClick={() => onRotate(credential.id)}
          >
            <RotateCw size={13} />
            轮换
          </RowAction>
          <RowAction
            label={`删除 ${credential.label}`}
            disabled={pending}
            tone="danger"
            onClick={() => onDelete(credential)}
          >
            <Trash2 size={13} />
            删除
          </RowAction>
        </div>
      </td>
    </tr>
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
      <p className="mt-1 text-[11px] text-success">
        可用 · HTTP {result.statusCode} · {result.latencyMs} ms
        {result.authErrorCleared ? " · 已清除认证错误" : ""}
      </p>
    );
  }
  if (result.reachable) {
    return (
      <p className="mt-1 text-[11px] text-warning">
        已连接 · HTTP {result.statusCode} · 凭据未通过
      </p>
    );
  }
  return (
    <p className="mt-1 text-[11px] text-danger">
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

function RowAction({
  label,
  children,
  disabled,
  onClick,
  tone = "accent",
}: {
  label: string;
  children: React.ReactNode;
  disabled?: boolean;
  onClick: () => void;
  tone?: "accent" | "danger";
}) {
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "focus-ring inline-flex h-7 items-center gap-1 rounded-[7px] px-2 text-[12px] font-medium transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        tone === "danger"
          ? "text-danger hover:bg-danger/8"
          : "text-secondary hover:bg-surface-muted hover:text-primary",
      )}
    >
      {children}
    </button>
  );
}

function Badge({
  children,
  tone = "neutral",
}: {
  children: React.ReactNode;
  tone?: "neutral" | "success";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center rounded-md px-1.5 py-0.5 text-[11px] font-medium",
        tone === "success" && "bg-success/10 text-success",
        tone === "neutral" && "bg-surface-muted text-secondary",
      )}
    >
      {children}
    </span>
  );
}
