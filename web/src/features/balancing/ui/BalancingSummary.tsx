import type { BalancingRuntime } from "../api/balancing-contracts";

export function BalancingSummary({ runtime }: { runtime: BalancingRuntime }) {
  return (
    <div className="border-t border-subtle">
      <dl className="grid sm:grid-cols-2 xl:grid-cols-4">
        <Metric label="近 60 秒请求" value={formatCount(runtime.totals.requestsInWindow)} />
        <Metric label="处理中" value={formatCount(runtime.totals.inFlight)} />
        <Metric label="排队" value={`${formatCount(runtime.queue.waiting)} / ${formatCount(runtime.queue.maxWaiting)}`} />
        <Metric label="RPM 已用尽" value={`${formatCount(runtime.totals.rateLimitedCredentialCount)} / ${formatCount(runtime.totals.limitedCredentialCount)}`} />
      </dl>

      <div className="border-t border-subtle px-5 py-4">
        <p className="text-xs leading-5 text-secondary">
          {formatCount(runtime.totals.enabledCredentialCount)} / {formatCount(runtime.totals.credentialCount)} 个账号已启用 · {formatCount(runtime.totals.limitedCredentialCount)} 个设置 RPM · 固定等待 {formatCount(runtime.totals.fixedWaiters)} · 累计选中 {formatCount(runtime.totals.selected)}
        </p>
        {runtime.providers.length > 0 ? (
          <div className="mt-3 grid gap-2 sm:grid-cols-2">
            {runtime.providers.map((provider) => (
              <div key={provider.providerKind} className="rounded-[10px] bg-surface-muted/60 px-3.5 py-3">
                <div className="flex items-center justify-between gap-4">
                  <p className="text-sm font-semibold">{providerLabel(provider.providerKind)}</p>
                  <p className="text-xs tabular-nums text-secondary">{formatCount(provider.requestsInWindow)} 次</p>
                </div>
                <p className="mt-1.5 text-xs leading-5 text-tertiary">
                  已启用 {formatCount(provider.enabledCredentialCount)} / {formatCount(provider.credentialCount)} · RPM 用尽 {formatCount(provider.rateLimitedCredentialCount)} · 处理中 {formatCount(provider.inFlight)}
                </p>
              </div>
            ))}
          </div>
        ) : (
          <p className="mt-3 text-sm text-tertiary">尚未配置可路由账号。</p>
        )}
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="border-b border-subtle px-5 py-4 last:border-b-0 sm:border-r sm:[&:nth-child(2)]:border-r-0 xl:border-b-0 xl:[&:nth-child(2)]:border-r xl:last:border-r-0">
      <dt className="text-xs text-secondary">{label}</dt>
      <dd className="mt-1 text-xl font-semibold tabular-nums">{value}</dd>
    </div>
  );
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}

function providerLabel(provider: "codex" | "claude" | "grok") {
  return { codex: "Codex", claude: "Claude", grok: "Grok" }[provider];
}
