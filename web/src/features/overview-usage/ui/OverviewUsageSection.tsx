import { RefreshCw } from "lucide-react";
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
  overviewRangeLabel,
} from "../model/overview-usage-presentation";
import { useOverviewUsage } from "../model/use-overview-usage";
import { OverviewModelChart } from "./OverviewModelChart";
import { OverviewTimeChart } from "./OverviewTimeChart";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";

export function OverviewUsageSection() {
  const [searchParams, setSearchParams] = useSearchParams();
  const rangeParam = searchParams.get("range");
  const range: OverviewUsageRange = isOverviewUsageRange(rangeParam) ? rangeParam : "24h";
  const query = useOverviewUsage(range);

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
      <section className="border-t border-subtle py-6 text-sm text-secondary" aria-busy="true">
        正在汇总调用与 Token 记录
      </section>
    );
  }
  if (!query.data) {
    return (
      <section className="border-t border-subtle py-6" role="alert">
        <h2 className="font-semibold">调用统计</h2>
        <p className="mt-2 text-sm text-secondary">{getOverviewUsageErrorMessage(query.error)}</p>
        <Button className="mt-4" onClick={() => void query.refetch()} disabled={query.isFetching}>
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
    <section className="border-t border-subtle py-6" aria-busy={query.isFetching}>
      <header className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
        <div>
          <h2 className="font-semibold">调用统计</h2>
          <p className="mt-1 text-xs leading-5 text-secondary">
            来自本地 RequestLog，当前显示近 {overviewRangeLabel(range)}。
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <SegmentedControl
            label="统计时间范围"
            options={OVERVIEW_RANGE_OPTIONS}
            selected={range}
            onSelect={setRange}
          />
          <Button variant="ghost" onClick={() => void query.refetch()} disabled={query.isFetching}>
            <RefreshCw size={14} className={query.isFetching ? "animate-spin" : undefined} />
            刷新
          </Button>
        </div>
      </header>

      {query.isError ? (
        <p className="mt-3 border-l-2 border-warning pl-3 text-xs text-secondary" role="status">
          刷新失败，仍显示最近数据：{getOverviewUsageErrorMessage(query.error)}
        </p>
      ) : null}

      <dl className="mt-5 grid grid-cols-3 border-y border-subtle [&>div]:border-r [&>div]:border-subtle [&>div:last-child]:border-r-0">
        <OverviewMetric
          label="请求数"
          value={formatOverviewInteger(overview.selected.requestCount)}
          note={`${formatOverviewInteger(overview.selected.successfulRequestCount)} 成功 · ${formatOverviewInteger(overview.selected.failedRequestCount)} 失败`}
        />
        <OverviewMetric
          label="Token 总消耗"
          value={formatOverviewInteger(overview.selected.totalTokens)}
          note={`usage 覆盖 ${formatOverviewInteger(overview.selected.tokenUsageRequestCount)} / ${formatOverviewInteger(overview.selected.requestCount)} 次请求`}
        />
        <OverviewMetric
          label="平均 RPM"
          value={formatOverviewRpm(averageRpm)}
          note={`${formatOverviewInteger(overview.selected.requestCount)} 次 ÷ ${formatOverviewInteger(rangeMinutes)} 分钟`}
        />
      </dl>

      <div className="mt-6 grid min-w-0 lg:grid-cols-[minmax(0,1fr)_18rem]">
        <section className="min-w-0 pb-6 lg:pb-0 lg:pr-6">
          <h3 className="text-sm font-semibold">调用趋势</h3>
          <p className="mt-1 text-xs text-secondary">
            近 {overviewRangeLabel(range)} · {formatOverviewInteger(overview.selected.requestCount)} 次调用 · 成功 {formatOverviewInteger(overview.selected.successfulRequestCount)} · 失败 {formatOverviewInteger(overview.selected.failedRequestCount)}
          </p>
          <OverviewTimeChart buckets={overview.timeBuckets} range={range} />
        </section>
        <section className="min-w-0 border-t border-subtle pt-6 lg:border-l lg:border-t-0 lg:pl-6 lg:pt-0">
          <h3 className="text-sm font-semibold">模型分布</h3>
          <p className="mt-1 text-xs text-secondary">
            {formatOverviewInteger(overview.models.length)} 个模型 · {formatOverviewInteger(overview.selected.requestCount)} 次调用
          </p>
          <OverviewModelChart models={overview.models} />
        </section>
      </div>
    </section>
  );
}

function OverviewMetric({ label, value, note }: { label: string; value: string; note: string }) {
  return (
    <div className="min-w-0 px-2.5 py-4 sm:px-4">
      <dt className="text-[11px] text-secondary">{label}</dt>
      <dd className="mt-1 truncate text-lg font-semibold tabular-nums sm:text-2xl" title={value}>
        {value}
      </dd>
      <p className="mt-1 truncate text-[10px] text-tertiary" title={note}>
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
    <div className="flex items-center rounded-[9px] bg-surface-muted p-0.5" role="group" aria-label={label}>
      {options.map((option) => {
        return (
          <button
            key={option.value}
            type="button"
            className={cn(
              "focus-ring inline-flex h-7 items-center gap-1.5 rounded-[7px] px-2.5 text-[11px] font-medium transition-colors",
              selected === option.value
                ? "bg-surface text-primary shadow-hairline"
                : "text-secondary hover:text-primary",
            )}
            aria-pressed={selected === option.value}
            onClick={() => onSelect(option.value)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
