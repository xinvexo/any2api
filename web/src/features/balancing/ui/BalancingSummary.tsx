import type { BalancingRuntime } from "../api/balancing-contracts";

export function BalancingSummary({ runtime }: { runtime: BalancingRuntime }) {
  return (
    <div className="mt-5 space-y-5">
      <dl className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Metric label="近 60 秒请求" value={formatCount(runtime.totals.requestsInWindow)} />
        <Metric label="处理中" value={formatCount(runtime.totals.inFlight)} />
        <Metric
          label="排队"
          value={`${formatCount(runtime.queue.waiting)} / ${formatCount(runtime.queue.maxWaiting)}`}
        />
        <Metric
          label="RPM 用尽账号"
          value={`${formatCount(runtime.totals.rateLimitedCredentialCount)} / ${formatCount(runtime.totals.limitedCredentialCount)}`}
        />
      </dl>

      <div>
        <p className="text-xs leading-5 text-secondary">
          {formatCount(runtime.totals.enabledCredentialCount)} /{" "}
          {formatCount(runtime.totals.credentialCount)} 个账号已启用 ·{" "}
          {formatCount(runtime.totals.limitedCredentialCount)} 个设置 RPM · 固定等待{" "}
          {formatCount(runtime.totals.fixedWaiters)} · 累计选中{" "}
          {formatCount(runtime.totals.selected)}
        </p>

        {runtime.providers.length > 0 ? (
          <ul className="mt-4 space-y-2" aria-label="Provider 调度汇总">
            {runtime.providers.map((provider) => (
              <li
                key={provider.providerKind}
                className="flex items-start justify-between gap-4 rounded-[12px] bg-surface-muted px-4 py-3"
              >
                <div className="min-w-0">
                  <p className="text-sm font-semibold tracking-tight">
                    {providerLabel(provider.providerKind)}
                  </p>
                  <p className="mt-1 text-xs leading-5 text-tertiary">
                    已启用 {formatCount(provider.enabledCredentialCount)} /{" "}
                    {formatCount(provider.credentialCount)} · RPM 用尽{" "}
                    {formatCount(provider.rateLimitedCredentialCount)} · 处理中{" "}
                    {formatCount(provider.inFlight)}
                  </p>
                </div>
                <p className="shrink-0 pt-0.5 text-sm font-semibold tabular-nums text-primary">
                  {formatCount(provider.requestsInWindow)}
                  <span className="ml-1 text-xs font-medium text-tertiary">次</span>
                </p>
              </li>
            ))}
          </ul>
        ) : (
          <p className="mt-4 text-sm text-tertiary">尚未配置可路由账号。</p>
        )}
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0 rounded-[12px] bg-surface-muted px-4 py-3.5">
      <dt className="text-xs font-medium text-secondary">{label}</dt>
      <dd className="mt-2 text-xl font-semibold tracking-tight tabular-nums">{value}</dd>
    </div>
  );
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}

function providerLabel(provider: "codex" | "claude" | "grok") {
  return { codex: "Codex", claude: "Claude", grok: "Grok" }[provider];
}
