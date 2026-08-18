import type {
  OverviewUsageModel,
  OverviewUsageRange,
  OverviewUsageTimeBucket,
} from "../api/overview-usage-contracts";
import { formatOverviewInteger } from "../model/overview-usage-presentation";
import { OverviewModelChart } from "./OverviewModelChart";
import { OverviewTimeChart } from "./OverviewTimeChart";

export function OverviewCharts({
  failedRequestCount,
  models,
  range,
  requestCount,
  successfulRequestCount,
  timeBuckets,
}: {
  failedRequestCount: number;
  models: OverviewUsageModel[];
  range: OverviewUsageRange;
  requestCount: number;
  successfulRequestCount: number;
  timeBuckets: OverviewUsageTimeBucket[];
}) {
  return (
    <div className="mt-6 grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_20rem] lg:items-stretch">
      <section className="flex min-w-0 flex-col rounded-[14px] bg-surface-muted/45 p-4">
        <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
          <h3 className="text-sm font-semibold tracking-tight">调用趋势</h3>
          <p className="text-xs tabular-nums text-secondary">
            {formatOverviewInteger(requestCount)} 次 · 成功{" "}
            {formatOverviewInteger(successfulRequestCount)} · 失败{" "}
            {formatOverviewInteger(failedRequestCount)}
          </p>
        </div>
        <div className="mt-3 flex-1 min-w-0">
          <OverviewTimeChart buckets={timeBuckets} range={range} />
        </div>
      </section>

      <section className="flex min-w-0 flex-col rounded-[14px] bg-surface-muted/45 p-4">
        <div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
          <h3 className="text-sm font-semibold tracking-tight">模型分布</h3>
          <p className="text-xs tabular-nums text-secondary">
            {formatOverviewInteger(models.length)} 个模型
          </p>
        </div>
        <div className="mt-3 flex-1 min-w-0">
          <OverviewModelChart models={models} />
        </div>
      </section>
    </div>
  );
}
