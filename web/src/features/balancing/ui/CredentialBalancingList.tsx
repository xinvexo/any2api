import { KeyRound } from "lucide-react";

import type { BalancingCredential, HealthState } from "../api/balancing-contracts";
import { Surface } from "@/shared/ui/Surface";

export function CredentialBalancingList({ credentials }: { credentials: BalancingCredential[] }) {
  if (credentials.length === 0) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-center">
        <div>
          <KeyRound size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 font-semibold">还没有路由 Credential</p>
          <p className="mt-2 text-sm text-secondary">先添加 Provider API Key 或 OAuth 账号，运行态才会出现在这里。</p>
        </div>
      </Surface>
    );
  }
  const totalSelections = credentials.reduce((total, item) => total + item.counters.selected, 0);
  return (
    <Surface className="divide-y divide-subtle overflow-hidden">
      {credentials.map((credential) => <CredentialRow key={credential.credentialId} credential={credential} totalSelections={totalSelections} />)}
    </Surface>
  );
}

function CredentialRow({ credential, totalSelections }: { credential: BalancingCredential; totalSelections: number }) {
  const rpmUsage = credential.requestsPerMinute === null
    ? null
    : Math.min(100, Math.round((credential.requestsInWindow / credential.requestsPerMinute) * 100));
  const counters = credential.counters;
  return (
    <article className="p-5 sm:p-6">
      <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="break-words font-semibold [overflow-wrap:anywhere]">{credential.label}</h2>
            <span className="rounded-full bg-surface-muted px-2.5 py-1 text-xs text-secondary">{providerLabel(credential.providerKind)}</span>
            {!credential.enabled || credential.authenticationExpired || !credential.endpointEnabled || !credential.proxyEnabled ? <span className="rounded-full bg-warning/15 px-2.5 py-1 text-xs text-warning-copy">已停用</span> : null}
          </div>
          <p className="mt-2 text-sm text-secondary">{credential.endpointName ?? "Provider OAuth"} · {credential.proxyName} ({credential.proxyKind.toUpperCase()})</p>
          <p className="mt-1 truncate font-mono text-xs text-tertiary" title={credential.credentialId}>{credential.credentialId}</p>
        </div>
        <div className="w-full lg:max-w-xs">
          <div className="flex items-center justify-between text-sm">
            <span className="text-secondary">RPM 窗口</span>
            <span className="font-semibold tabular-nums">
              {credential.requestsPerMinute === null
                ? "无限制"
                : `${credential.requestsInWindow} / ${credential.requestsPerMinute}`}
            </span>
          </div>
          {rpmUsage === null ? (
            <p className="mt-2 text-xs text-tertiary">未启用本地请求频率限制</p>
          ) : (
            <div className="mt-2 h-2 overflow-hidden rounded-full bg-surface-muted" role="progressbar" aria-label={`${credential.label} RPM 使用率`} aria-valuenow={rpmUsage} aria-valuemin={0} aria-valuemax={100}>
              <div className="h-full rounded-full bg-accent" style={{ width: `${rpmUsage}%` }} />
            </div>
          )}
          <p className="mt-2 text-xs text-tertiary">{rpmDetail(credential)} · 处理中 {credential.inFlight} · 固定等待 {credential.fixedWaiters}</p>
        </div>
      </div>

      <div className="mt-5 grid gap-2 text-xs text-secondary sm:grid-cols-2 xl:grid-cols-3">
        <Counter label="选中" value={counters.selected} detail={totalSelections === 0 ? "0%" : `${Math.round((counters.selected / totalSelections) * 100)}%`} />
        <Counter label="RPM 过滤" value={counters.filteredRateLimit} />
        <Counter label="Credential 健康过滤" value={counters.filteredCredentialHealth} />
        <Counter label="Endpoint 健康过滤" value={counters.filteredEndpointHealth} />
        <Counter label="Proxy 健康过滤" value={counters.filteredProxyHealth} />
      </div>

      <div className="mt-5 border-t border-subtle pt-4">
        {credential.models.length === 0 ? (
          <p className="text-sm text-secondary">当前没有启用的模型路由引用这个 Endpoint。</p>
        ) : (
          <div className="space-y-3">
            {credential.models.map((model) => (
              <div key={model.upstreamModel} className="grid gap-3 rounded-control bg-surface-muted/60 px-4 py-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center">
                <p className="min-w-0 break-all text-sm font-medium">{model.upstreamModel}</p>
                <div className="flex flex-wrap gap-2">
                  <Health label="Credential" state={model.credential} />
                  <Health label="Endpoint" state={model.endpoint} />
                  <Health label="Proxy" state={model.proxy} />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </article>
  );
}

function rpmDetail(credential: BalancingCredential) {
  if (credential.requestsPerMinute === null) return "RPM 无限制";
  if (credential.remainingRequests === 0) {
    return `${Math.ceil((credential.retryInMs ?? 0) / 1_000)} 秒后可用`;
  }
  return `剩余 ${credential.remainingRequests ?? 0} 次`;
}

function Counter({ label, value, detail }: { label: string; value: number; detail?: string }) {
  return <div className="flex items-center justify-between gap-3 rounded-control bg-surface-muted/60 px-3 py-2"><span>{label}</span><span className="font-semibold tabular-nums text-primary">{value}{detail ? ` · ${detail}` : ""}</span></div>;
}

function Health({ label, state }: { label: string; state: HealthState }) {
  const tone = state.status === "available" ? "bg-success/15 text-success-copy" : state.status === "cooling" ? "bg-warning/15 text-warning-copy" : "bg-danger/15 text-danger-copy";
  const value = state.status === "available" ? "可用" : state.status === "unavailable" ? "不可用" : `${Math.ceil((state.retryInMs ?? 0) / 1_000)}s`;
  return <span className={`rounded-full px-2.5 py-1 text-xs ${tone}`}>{label} {value}</span>;
}

function providerLabel(provider: BalancingCredential["providerKind"]) {
  return provider === "codex" ? "Codex" : "Claude";
}
