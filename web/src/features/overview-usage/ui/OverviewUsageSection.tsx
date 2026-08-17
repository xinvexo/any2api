import {
  Activity,
  CheckCircle2,
  Coins,
  DatabaseZap,
  Gauge,
  RefreshCw,
  type LucideIcon,
} from "lucide-react";
import { Suspense, lazy } from "react";
import { useSearchParams } from "react-router-dom";

import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { Skeleton } from "@/shared/ui/Skeleton";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

import {
  isOverviewUsageRange,
  type OverviewUsageRange,
} from "../api/overview-usage-contracts";
import { getOverviewUsageErrorMessage } from "../model/overview-usage-error";
import {
  calculateOverviewAverageRpm,
  calculateOverviewCacheHitRate,
  formatOverviewInteger,
  formatOverviewRpm,
  OVERVIEW_RANGE_OPTIONS,
} from "../model/overview-usage-presentation";
import { useOverviewUsage } from "../model/use-overview-usage";

const OverviewCharts = lazy(() =>
  import("./OverviewCharts").then((module) => ({ default: module.OverviewCharts })),
);

export function OverviewUsageSection() {
  const [searchParams, setSearchParams] = useSearchParams();
  const rangeParam = searchParams.get("range");
  const range: OverviewUsageRange = isOverviewUsageRange(rangeParam) ? rangeParam : "24h";
  const query = useOverviewUsage(range);

  async function refreshUsage() {
    const result = await query.refetch();
    if (result.isSuccess) {
      notify.success("用量概览已刷新");
    }
  }

  function setRange(value: OverviewUsageRange) {
    setSearchParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.set("range", value);
        next.delete("view");
        return next;
      },
      { replace: true },
    );
  }

  if (query.isPending && !query.data) {
    return (
      <section className="border-t border-subtle pt-8 text-sm text-secondary" aria-busy="true">
        正在汇总调用与 Token 记录
      </section>
    );
  }
  if (!query.data) {
    return (
      <section className="border-t border-subtle pt-8" role="alert">
        <h2 className="text-base font-semibold tracking-tight">调用分析</h2>
        <p className="mt-2 text-sm leading-6 text-secondary">
          {getOverviewUsageErrorMessage(query.error)}
        </p>
        <Button className="mt-4" onClick={() => void refreshUsage()} disabled={query.isFetching}>
          <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
          重试
        </Button>
      </section>
    );
  }

  const overview = query.data;
  const rangeMinutes = (overview.rangeEndedAtMs - overview.rangeStartedAtMs) / 60_000;
  const averageRpm = calculateOverviewAverageRpm(
    overview.selected.requestCount,
    overview.rangeStartedAtMs,
    overview.rangeEndedAtMs,
  );
  const successRate = calculateSuccessRate(
    overview.selected.successfulRequestCount,
    overview.selected.requestCount,
  );
  const cacheHitRate = calculateOverviewCacheHitRate(
    overview.selected.cacheReadTokens,
    overview.selected.inputTokens,
  );

  return (
    <section className="min-w-0 border-t border-subtle pt-8" aria-busy={query.isFetching}>
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span
              className="grid size-8 place-items-center rounded-[8px] bg-accent/10 text-accent"
              aria-hidden="true"
            >
              <Activity size={17} strokeWidth={2.2} />
            </span>
            <h2 className="text-base font-semibold tracking-tight">调用分析</h2>
          </div>
          <p className="mt-2 text-xs text-tertiary">按时间范围查看请求质量与调用节奏</p>
        </div>
        <SegmentedControl
          label="统计时间范围"
          options={OVERVIEW_RANGE_OPTIONS}
          selected={range}
          onSelect={setRange}
        />
      </header>

      {query.isError ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          刷新失败，仍显示最近数据：{getOverviewUsageErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="mt-6 grid min-w-0 divide-y divide-subtle border-y border-subtle lg:grid-cols-5 lg:divide-x lg:divide-y-0 lg:divide-subtle">
        <OverviewMetric
          icon={Activity}
          label="请求数"
          value={formatOverviewInteger(overview.selected.requestCount)}
          note={`${formatOverviewInteger(overview.selected.successfulRequestCount)} 成功 · ${formatOverviewInteger(overview.selected.failedRequestCount)} 失败`}
          tone="blue"
        />
        <OverviewMetric
          icon={CheckCircle2}
          label="成功率"
          value={successRate === null ? "—" : formatOverviewPercent(successRate)}
          note={
            successRate === null
              ? "暂无请求"
              : `${formatOverviewInteger(overview.selected.successfulRequestCount)} / ${formatOverviewInteger(overview.selected.requestCount)} 次`
          }
          tone="green"
        />
        <OverviewMetric
          icon={Coins}
          label="Token 总消耗"
          value={formatOverviewInteger(overview.selected.totalTokens)}
          note={`usage 覆盖 ${formatOverviewInteger(overview.selected.tokenUsageRequestCount)} 次`}
          tone="violet"
        />
        <OverviewMetric
          icon={DatabaseZap}
          label="缓存命中率"
          value={cacheHitRate === null ? "—" : formatOverviewPercent(cacheHitRate)}
          note={
            cacheHitRate === null
              ? "暂无输入 Token"
              : `缓存读取 ${formatOverviewInteger(overview.selected.cacheReadTokens)} / 输入 ${formatOverviewInteger(overview.selected.inputTokens)}`
          }
          tone="cyan"
        />
        <OverviewMetric
          icon={Gauge}
          label="平均 RPM"
          value={formatOverviewRpm(averageRpm)}
          note={`${formatOverviewInteger(overview.selected.requestCount)} 次 ÷ ${formatOverviewInteger(rangeMinutes)} 分钟`}
          tone="orange"
        />
      </dl>

      <Suspense fallback={<OverviewChartsLoading />}>
        <OverviewCharts
          failedRequestCount={overview.selected.failedRequestCount}
          models={overview.models}
          range={range}
          requestCount={overview.selected.requestCount}
          successfulRequestCount={overview.selected.successfulRequestCount}
          timeBuckets={overview.timeBuckets}
        />
      </Suspense>
    </section>
  );
}

export function OverviewChartsLoading() {
  return (
    <div
      className="mt-6 grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]"
      role="status"
      aria-label="正在加载调用图表"
      aria-live="polite"
    >
      <section className="flex min-w-0 flex-col rounded-[8px] border border-subtle bg-surface/45 p-4">
        <Skeleton className="h-4 w-48" />
        <div className="mt-4 h-64">
          <Skeleton className="h-full w-full rounded-[6px]" />
        </div>
      </section>
      <section className="flex min-w-0 flex-col rounded-[8px] border border-subtle bg-surface/45 p-4">
        <Skeleton className="h-4 w-28" />
        <div className="mt-4 h-64">
          <Skeleton className="h-full w-full rounded-[6px]" />
        </div>
      </section>
    </div>
  );
}

function OverviewMetric({
  icon: Icon,
  label,
  value,
  note,
  tone,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  note: string;
  tone: "blue" | "green" | "violet" | "cyan" | "orange";
}) {
  const color = {
    blue: "var(--chart-1)",
    green: "var(--chart-6)",
    violet: "var(--chart-2)",
    cyan: "var(--chart-7)",
    orange: "var(--chart-5)",
  }[tone];
  return (
    <div className="min-w-0 py-4 first:pt-4 last:pb-4 sm:px-4 sm:first:pl-0 sm:last:pr-0">
      <div className="flex items-center gap-2">
        <Icon size={14} style={{ color }} aria-hidden="true" />
        <dt className="truncate text-xs font-medium text-secondary">{label}</dt>
      </div>
      <dd className="mt-2 truncate text-[1.65rem] font-semibold leading-none tracking-tight tabular-nums" title={value}>
        {value}
      </dd>
      <p className="mt-2 truncate text-[11px] leading-4 text-tertiary" title={note}>
        {note}
      </p>
    </div>
  );
}

function SegmentedControl<T extends string>({
  label,
  options,
  selected,
  onSelect,
}: {
  label: string;
  options: ReadonlyArray<{ value: T; label: string }>;
  selected: T;
  onSelect: (value: T) => void;
}) {
  return (
    <div
      className="relative isolate flex w-fit items-center rounded-[8px] bg-surface-muted p-0.5"
      role="group"
      aria-label={label}
    >
      <SlidingSelectionIndicator
        selected={selected}
        className="rounded-[6px] bg-surface shadow-hairline"
      />
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={cn(
            "focus-ring relative z-10 inline-flex h-7 items-center gap-1.5 rounded-[6px] px-2.5 text-[11px] font-medium transition-colors",
            selected === option.value ? "text-primary" : "text-secondary hover:text-primary",
          )}
          data-sliding-selection-item={option.value}
          aria-pressed={selected === option.value}
          onClick={() => onSelect(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function calculateSuccessRate(successful: number, total: number) {
  return total > 0 ? (successful / total) * 100 : null;
}

function formatOverviewPercent(value: number) {
  return `${new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 1 }).format(value)}%`;
}
