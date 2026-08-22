import { Activity, CircleAlert, KeyRound, ListFilter, Radio } from "lucide-react";

import type { BalancingRuntime } from "../api/balancing-contracts";

import { OverviewMetricTile, ProgressBar } from "./OverviewMetricTile";

export function LiveLoadPanel({ runtime }: { runtime: BalancingRuntime | undefined }) {
  const requests = runtime ? formatCount(runtime.totals.requestsInWindow) : "—";
  const queueRatio = runtime ? ratioPercent(runtime.queue.waiting, runtime.queue.maxWaiting) : null;
  const enabledCredentialRatio = runtime
    ? ratioPercent(runtime.totals.enabledCredentialCount, runtime.totals.credentialCount)
    : null;
  const limitedRatio = runtime
    ? ratioPercent(runtime.totals.rateLimitedCredentialCount, runtime.totals.limitedCredentialCount)
    : null;
  const hasExhaustedCredentials = (runtime?.totals.rateLimitedCredentialCount ?? 0) > 0;

  return (
    <section className="min-w-0" aria-labelledby="overview-load-title">
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
          <p className="mt-0.5 truncate text-xs text-tertiary">最近一分钟的处理与排队情况</p>
        </div>
      </div>

      <div className="mt-3 grid min-w-0 grid-cols-1 gap-2.5 min-[360px]:grid-cols-2">
        <OverviewMetricTile
          icon={Radio}
          label="近 60 秒请求"
          value={requests}
          note="最近一分钟内的请求量"
          tone="blue"
          valueTone="accent"
        />
        <OverviewMetricTile
          icon={Activity}
          label="进行中请求"
          value={runtime ? formatCount(runtime.totals.inFlight) : "—"}
          note="当前尚未结束"
          tone="violet"
        />
        <OverviewMetricTile
          icon={ListFilter}
          label="等待中请求"
          value={runtime ? formatCount(runtime.queue.waiting) : "—"}
          note={runtime ? `最多等待 ${formatCount(runtime.queue.maxWaiting)} 个` : "等待快照"}
          progress={queueRatio}
          progressLabel="等待中请求占上限"
          tone="orange"
        />
        <OverviewMetricTile
          icon={KeyRound}
          label="账号与密钥"
          value={
            runtime
              ? `${formatCount(runtime.totals.enabledCredentialCount)} / ${formatCount(runtime.totals.credentialCount)}`
              : "—"
          }
          note="已启用 / 总数"
          progress={enabledCredentialRatio}
          progressLabel="已启用账号与密钥占比"
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
          <span className="truncate text-xs font-medium">已达每分钟上限</span>
          <strong className="shrink-0 text-sm font-semibold tabular-nums">{value}</strong>
        </div>
        <ProgressBar value={meter} color="var(--danger)" label="达到每分钟上限的账号与密钥占比" />
        <p className="mt-1 text-[10px] leading-4 text-tertiary">稍后会自动恢复</p>
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
