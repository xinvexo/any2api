import { RefreshCw } from "lucide-react";
import { Suspense, lazy } from "react";
import { useSearchParams } from "react-router-dom";

import {
  isOverviewUsageRange,
  type OverviewUsageRange,
} from "../api/overview-usage-contracts";
import { getOverviewUsageErrorMessage } from "../model/overview-usage-error";
import {
  calculateOverviewAverageRpm,
  formatOverviewInteger,
  formatOverviewRpm,
  OVERVIEW_RANGE_OPTIONS,
} from "../model/overview-usage-presentation";
import { useOverviewUsage } from "../model/use-overview-usage";
import { cn } from "@/shared/lib/cn";
import { notify } from "@/shared/notifications";
import { Button } from "@/shared/ui/Button";
import { Skeleton } from "@/shared/ui/Skeleton";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

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
      <section className="text-sm text-secondary" aria-busy="true">
        正在汇总调用与 Token 记录
      </section>
    );
  }
  if (!query.data) {
    return (
      <section role="alert">
        <h2 className="text-base font-semibold tracking-tight">调用统计</h2>
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

  return (
    <section className="min-w-0" aria-busy={query.isFetching}>
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div className="min-w-0">
          <h2 className="text-base font-semibold tracking-tight">调用统计</h2>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <SegmentedControl
            label="统计时间范围"
            options={OVERVIEW_RANGE_OPTIONS}
            selected={range}
            onSelect={setRange}
          />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refreshUsage()}
            disabled={query.isFetching}
          >
            <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </header>

      {query.isError ? (
        <p className="mt-4 border-l-2 border-warning pl-3 text-xs leading-5 text-secondary" role="status">
          刷新失败，仍显示最近数据：{getOverviewUsageErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="mt-5 grid gap-3 sm:grid-cols-3">
        <OverviewMetric
          label="请求数"
          value={formatOverviewInteger(overview.selected.requestCount)}
          note={`${formatOverviewInteger(overview.selected.successfulRequestCount)} 成功 · ${formatOverviewInteger(overview.selected.failedRequestCount)} 失败`}
        />
        <OverviewMetric
          label="Token 总消耗"
          value={formatOverviewInteger(overview.selected.totalTokens)}
          note={`usage 覆盖 ${formatOverviewInteger(overview.selected.tokenUsageRequestCount)} / ${formatOverviewInteger(overview.selected.requestCount)} 次`}
        />
        <OverviewMetric
          label="平均 RPM"
          value={formatOverviewRpm(averageRpm)}
          note={`${formatOverviewInteger(overview.selected.requestCount)} 次 ÷ ${formatOverviewInteger(rangeMinutes)} 分钟`}
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
      className="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_20rem] lg:items-stretch lg:gap-8"
      role="status"
      aria-label="正在加载调用图表"
      aria-live="polite"
    >
      <section className="flex min-w-0 flex-col">
        <Skeleton className="mb-3 h-4 w-48" />
        <div className="h-80 rounded-[12px] bg-surface-muted/70 p-4">
          <Skeleton className="h-full w-full rounded-[10px]" />
        </div>
      </section>
      <section className="flex min-w-0 flex-col">
        <Skeleton className="mb-3 h-4 w-28" />
        <div className="h-80 rounded-[12px] bg-surface-muted/70 p-4">
          <Skeleton className="h-full w-full rounded-[10px]" />
        </div>
      </section>
    </div>
  );
}

function OverviewMetric({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="min-w-0 rounded-[12px] bg-surface-muted px-4 py-4">
      <dt className="text-xs font-medium text-secondary">{label}</dt>
      <dd className="mt-2 truncate text-[1.75rem] font-semibold tracking-tight tabular-nums" title={value}>
        {value}
      </dd>
      <p className="mt-2 truncate text-xs leading-5 text-tertiary" title={note}>
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
      className="relative isolate flex items-center rounded-[9px] bg-surface-muted p-0.5"
      role="group"
      aria-label={label}
    >
      <SlidingSelectionIndicator
        selected={selected}
        className="rounded-[7px] bg-surface shadow-hairline"
      />
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={cn(
            "focus-ring relative z-10 inline-flex h-7 items-center gap-1.5 rounded-[7px] px-2.5 text-[11px] font-medium transition-colors",
            selected === option.value
              ? "text-primary"
              : "text-secondary hover:text-primary",
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
