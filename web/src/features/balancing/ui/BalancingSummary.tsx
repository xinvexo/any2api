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

      <dl className="grid grid-cols-2 gap-3 xl:grid-cols-4" aria-label="运行态观测">
        <Metric
          label="Transport 缓存命中 / 未命中"
          value={
            runtime.transport
              ? `${formatCount(runtime.transport.cacheHits)} / ${formatCount(runtime.transport.cacheMisses)}`
              : "-"
          }
          note={
            runtime.transport
              ? `本次运行累计 · 当前条目 ${formatCount(runtime.transport.cacheEntries)} / ${formatCount(runtime.transport.cacheCapacity)} · 淘汰 ${formatCount(runtime.transport.cacheEvictions)}`
              : undefined
          }
        />
        <Metric
          label="熔断器状态"
          value={`${runtime.breakers.closed} 闭合 · ${runtime.breakers.open} 打开 · ${runtime.breakers.halfOpen} 半开`}
        />
        <Metric
          label="遥测排队 / 容量"
          value={`${runtime.telemetry.queued} / ${runtime.telemetry.capacity}`}
          note={`写入中 ${runtime.telemetry.inFlight}，累计丢弃 ${runtime.telemetry.dropped}`}
        />
        <Metric label="停机阶段" value={shutdownLabel(runtime.process.shutdownPhase)} />
      </dl>
    </div>
  );
}

function Metric({ label, value, note }: { label: string; value: string; note?: string }) {
  return (
    <div className="min-w-0 rounded-[12px] bg-surface-muted px-4 py-3.5">
      <dt className="text-xs font-medium text-secondary">{label}</dt>
      <dd className="mt-2 text-xl font-semibold tracking-tight tabular-nums">{value}</dd>
      {note ? <p className="mt-1 text-[11px] leading-4 text-tertiary">{note}</p> : null}
    </div>
  );
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}

function providerLabel(provider: "codex" | "claude" | "grok" | "kimi") {
  return { codex: "Codex", claude: "Claude", grok: "Grok", kimi: "Kimi" }[
    provider
  ];
}

function shutdownLabel(phase: "running" | "draining" | "forced") {
  return { running: "运行中", draining: "排空中", forced: "强制停机" }[phase];
}
