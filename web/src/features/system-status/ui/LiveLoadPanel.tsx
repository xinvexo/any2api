import { Activity, CircleAlert, Database, ListFilter, Radio } from "lucide-react";

import type { BalancingRuntime } from "@/features/balancing";

import { OverviewMetricTile, ProgressBar } from "./OverviewMetricTile";

export function LiveLoadPanel({ runtime }: { runtime: BalancingRuntime | undefined }) {
  const live = runtime !== undefined;
  const pool = runtime?.transport;
  const requests = runtime ? formatCount(runtime.totals.requestsInWindow) : "—";
  const queueRatio = runtime ? ratioPercent(runtime.queue.waiting, runtime.queue.maxWaiting) : null;
  const poolRatio = pool ? ratioPercent(pool.cacheEntries, pool.cacheCapacity) : null;
  const limitedRatio = runtime
    ? ratioPercent(runtime.totals.rateLimitedCredentialCount, runtime.totals.limitedCredentialCount)
    : null;
  const hasExhaustedCredentials = (runtime?.totals.rateLimitedCredentialCount ?? 0) > 0;

  return (
    <section className="min-w-0" aria-labelledby="overview-load-title">
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <span
            className="grid size-8 shrink-0 place-items-center rounded-[8px] bg-[color-mix(in_srgb,var(--chart-6)_13%,transparent)] text-[var(--chart-6)]"
            aria-hidden="true"
          >
            <Activity size={17} strokeWidth={2.2} />
          </span>
          <div className="min-w-0">
            <h2 id="overview-load-title" className="text-sm font-semibold tracking-tight">
              请求负载
            </h2>
            <p className="mt-0.5 truncate text-xs text-tertiary">实时窗口与资源压力</p>
          </div>
        </div>
        <span
          className={
            live
              ? "inline-flex shrink-0 items-center gap-1.5 rounded-full bg-success/10 px-2 py-1 text-[10px] font-medium text-success"
              : "inline-flex shrink-0 items-center gap-1.5 rounded-full bg-surface-muted px-2 py-1 text-[10px] font-medium text-tertiary"
          }
        >
          <span className="size-1.5 rounded-full bg-current" aria-hidden="true" />
          {live ? "实时" : "等待"}
        </span>
      </div>

      <div className="mt-3 grid min-w-0 grid-cols-1 gap-2.5 min-[360px]:grid-cols-2">
        <OverviewMetricTile
          icon={Radio}
          label="近 60 秒请求"
          value={requests}
          note="RPM"
          tone="blue"
          valueTone="accent"
        />
        <OverviewMetricTile
          icon={Activity}
          label="活动上游"
          value={runtime ? formatCount(runtime.totals.inFlight) : "—"}
          note="当前正在执行"
          tone="violet"
        />
        <OverviewMetricTile
          icon={ListFilter}
          label="排队等待"
          value={runtime ? formatCount(runtime.queue.waiting) : "—"}
          note={runtime ? `上限 ${formatCount(runtime.queue.maxWaiting)}` : "等待快照"}
          progress={queueRatio}
          progressLabel="排队等待占上限"
          tone="orange"
        />
        <OverviewMetricTile
          icon={Database}
          label="Transport 客户端"
          value={pool ? `${formatCount(pool.cacheEntries)} / ${formatCount(pool.cacheCapacity)}` : "—"}
          note="共享客户端缓存 · 非 socket"
          progress={poolRatio}
          progressLabel="客户端池占用"
          tone="green"
        />
      </div>
      {runtime && hasExhaustedCredentials ? (
        <div className="mt-1 border-t border-danger/25 pt-3">
          <LoadAlert
            value={`${formatCount(runtime.totals.rateLimitedCredentialCount)} / ${formatCount(runtime.totals.limitedCredentialCount)}`}
            meter={limitedRatio}
          />
        </div>
      ) : null}
    </section>
  );
}

function LoadAlert({ value, meter }: { value: string; meter: number | null }) {
  return (
    <div className="flex min-w-0 items-start gap-2.5 text-danger">
      <CircleAlert size={15} className="mt-0.5 shrink-0" strokeWidth={2} aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <span className="truncate text-xs font-medium">RPM 用尽</span>
          <strong className="shrink-0 text-sm font-semibold tabular-nums">{value}</strong>
        </div>
        <ProgressBar value={meter} color="var(--danger)" label="RPM 用尽凭据占比" />
        <p className="mt-1 text-[10px] leading-4 text-tertiary">达到本地限制的凭据</p>
      </div>
    </div>
  );
}

function ratioPercent(value: number, total: number) {
  return total > 0 ? (value / total) * 100 : null;
}

function formatCount(value: number) {
  return value.toLocaleString("zh-CN");
}
