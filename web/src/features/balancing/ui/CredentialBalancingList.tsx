import { KeyRound } from "lucide-react";

import type { BalancingCredential, HealthState } from "../api/balancing-contracts";
import { Surface } from "@/shared/ui/Surface";

export function CredentialBalancingList({ credentials }: { credentials: BalancingCredential[] }) {
  if (credentials.length === 0) {
    return (
      <Surface className="flex min-h-56 items-center justify-center p-7 text-center">
        <div>
          <KeyRound size={23} className="mx-auto text-tertiary" aria-hidden="true" />
          <p className="mt-3 font-semibold">还没有 Provider Credential</p>
          <p className="mt-2 text-sm text-secondary">先在 Provider 页面添加 API Key，运行态容量才会出现在这里。</p>
        </div>
      </Surface>
    );
  }
  const totalSelections = credentials.reduce((total, item) => total + item.counters.selectedGeneration, 0);
  return (
    <Surface className="divide-y divide-subtle overflow-hidden">
      {credentials.map((credential) => <CredentialRow key={credential.credentialId} credential={credential} totalSelections={totalSelections} />)}
    </Surface>
  );
}

function CredentialRow({ credential, totalSelections }: { credential: BalancingCredential; totalSelections: number }) {
  const load = Math.min(100, Math.round((credential.inFlight / credential.maxConcurrency) * 100));
  const counters = credential.counters;
  return (
    <article className="p-5 sm:p-6">
      <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="break-words font-semibold [overflow-wrap:anywhere]">{credential.label}</h2>
            <span className="rounded-full bg-surface-muted px-2.5 py-1 text-xs text-secondary">{providerLabel(credential.providerKind)}</span>
            {!credential.enabled || !credential.endpointEnabled || !credential.proxyEnabled ? <span className="rounded-full bg-warning/15 px-2.5 py-1 text-xs text-warning-copy">已停用</span> : null}
          </div>
          <p className="mt-2 text-sm text-secondary">{credential.endpointName} · {credential.proxyName} ({credential.proxyKind.toUpperCase()})</p>
          <p className="mt-1 truncate font-mono text-xs text-tertiary" title={credential.credentialId}>{credential.credentialId}</p>
        </div>
        <div className="w-full lg:max-w-xs">
          <div className="flex items-center justify-between text-sm">
            <span className="text-secondary">当前负载</span>
            <span className="font-semibold tabular-nums">{credential.inFlight} / {credential.maxConcurrency}</span>
          </div>
          <div className="mt-2 h-2 overflow-hidden rounded-full bg-surface-muted" role="progressbar" aria-label={`${credential.label} 当前负载`} aria-valuenow={load} aria-valuemin={0} aria-valuemax={100}>
            <div className="h-full rounded-full bg-accent" style={{ width: `${load}%` }} />
          </div>
          <p className="mt-2 text-xs text-tertiary">固定等待 {credential.fixedWaiters} · 辅助占用 {credential.auxiliaryInFlight}</p>
        </div>
      </div>

      <div className="mt-5 grid gap-2 text-xs text-secondary sm:grid-cols-2 xl:grid-cols-3">
        <Counter label="生成选中" value={counters.selectedGeneration} detail={totalSelections === 0 ? "0%" : `${Math.round((counters.selectedGeneration / totalSelections) * 100)}%`} />
        <Counter label="辅助选中" value={counters.selectedAuxiliary} />
        <Counter label="满载过滤" value={counters.filteredCapacity} />
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
