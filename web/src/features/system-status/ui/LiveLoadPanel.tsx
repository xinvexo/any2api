import { Activity, CircleAlert, Database, ListFilter, Radio, type LucideIcon } from "lucide-react";

import type { BalancingRuntime } from "@/features/balancing";

import { ProgressBar } from "./LiveResourceGrid";

export function LiveLoadPanel({ runtime }: { runtime: BalancingRuntime | undefined }) {
  const live = runtime !== undefined;
  const pool = runtime?.transport;
  const requests = runtime ? formatCount(runtime.totals.requestsInWindow) : "—";
  const queueRatio = runtime ? ratioPercent(runtime.queue.waiting, runtime.queue.maxWaiting) : null;
  const poolRatio = pool ? ratioPercent(pool.cacheEntries, pool.cacheCapacity) : null;
  const limitedRatio = runtime
    ? ratioPercent(runtime.totals.rateLimitedCredentialCount, runtime.totals.limitedCredentialCount)
    : null;

  return (
    <section
      className="min-w-0 rounded-[8px] border border-subtle bg-surface/70 p-4 sm:p-5"
      aria-labelledby="overview-load-title"
    >
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

      <div className="mt-5 flex items-end justify-between gap-4 border-b border-subtle pb-4">
        <div className="min-w-0">
          <p className="text-xs text-secondary">近 60 秒请求</p>
          <div className="mt-1 flex items-baseline gap-2">
            <strong className="truncate text-[2.35rem] font-semibold leading-none tracking-tight tabular-nums">
              {requests}
            </strong>
            <span className="text-xs font-medium text-tertiary">RPM</span>
          </div>
        </div>
        <span
          className="grid size-11 shrink-0 place-items-center rounded-full bg-accent/10 text-accent"
          aria-hidden="true"
        >
          <Radio size={20} strokeWidth={2} />
        </span>
      </div>

      <dl className="divide-y divide-subtle">
        <LoadRow
          icon={Activity}
          label="活动上游"
          value={runtime ? formatCount(runtime.totals.inFlight) : "—"}
          note="当前正在执行"
        />
        <LoadRow
          icon={ListFilter}
          label="排队等待"
          value={runtime ? formatCount(runtime.queue.waiting) : "—"}
          note={runtime ? `上限 ${formatCount(runtime.queue.maxWaiting)}` : "等待快照"}
          meter={queueRatio}
          meterLabel="排队等待占上限"
          meterColor="var(--chart-5)"
        />
        <LoadRow
          icon={Database}
          label="客户端池条目"
          value={pool ? `${formatCount(pool.cacheEntries)} / ${formatCount(pool.cacheCapacity)}` : "—"}
          note="Transport 条目，不是 TCP socket"
          meter={poolRatio}
          meterLabel="客户端池占用"
          meterColor="var(--chart-7)"
        />
        {runtime && runtime.totals.limitedCredentialCount > 0 ? (
          <LoadRow
            icon={CircleAlert}
            label="RPM 用尽"
            value={`${formatCount(runtime.totals.rateLimitedCredentialCount)} / ${formatCount(runtime.totals.limitedCredentialCount)}`}
            note="受限凭据"
            meter={limitedRatio}
            meterLabel="RPM 用尽凭据占比"
            meterColor="var(--danger)"
            tone="danger"
          />
        ) : null}
      </dl>
    </section>
  );
}

function LoadRow({
  icon: Icon,
  label,
  value,
  note,
  meter,
  meterLabel,
  meterColor,
  tone = "neutral",
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  note: string;
  meter?: number | null;
  meterLabel?: string;
  meterColor?: string;
  tone?: "neutral" | "danger";
}) {
  return (
    <div className="flex min-w-0 items-center gap-3 py-3 first:pt-4 last:pb-0">
      <span
        className={tone === "danger" ? "text-danger" : "text-tertiary"}
        aria-hidden="true"
      >
        <Icon size={15} strokeWidth={2} />
      </span>
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-3">
          <dt className="truncate text-xs font-medium text-secondary">{label}</dt>
          <dd className="shrink-0 text-sm font-semibold tabular-nums">{value}</dd>
        </div>
        {meter !== undefined ? (
          <ProgressBar
            value={meter}
            color={meterColor}
            label={meterLabel ?? label}
          />
        ) : null}
        <p className="mt-1 truncate text-[10px] leading-4 text-tertiary" title={note}>
          {note}
        </p>
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
