import type { UpstreamRequestUsage } from "../api/upstream-request-usage";

export function RequestUsageStats({
  label,
  usage,
}: {
  label: string;
  usage: UpstreamRequestUsage;
}) {
  const successRate = usage.totalRequests
    ? (usage.successfulRequests / usage.totalRequests) * 100
    : null;
  const outcomeLabel = usage.recentOutcomes
    .map((outcome) => (isSuccess(outcome.statusCode) ? "成功" : `失败 ${outcome.statusCode}`))
    .join("、");

  return (
    <div className="min-w-[180px] space-y-1.5">
      <div className="flex flex-wrap items-center gap-1.5 text-[11px] tabular-nums">
        <span className="text-tertiary">请求 {formatCount(usage.totalRequests)}</span>
        <span className="rounded-md bg-success/10 px-1.5 py-0.5 font-medium text-success">
          成功 {formatCount(usage.successfulRequests)}
        </span>
        <span className="rounded-md bg-danger/10 px-1.5 py-0.5 font-medium text-danger">
          失败 {formatCount(usage.failedRequests)}
        </span>
      </div>
      {successRate === null ? (
        <p className="text-[11px] text-tertiary">暂无调用</p>
      ) : (
        <div className="flex items-center gap-2">
          <div
            className="flex min-w-0 flex-1 items-center gap-[3px]"
            role="img"
            aria-label={`${label} 最近 ${usage.recentOutcomes.length} 次调用：${outcomeLabel || "暂无结果"}`}
          >
            {usage.recentOutcomes.map((outcome, index) => (
              <span
                key={`${outcome.statusCode}-${index}`}
                className={`block size-[4px] shrink-0 rounded-[1px] ${outcomeTone(outcome.statusCode)}`}
                title={`HTTP ${outcome.statusCode}`}
              />
            ))}
          </div>
          <span className="shrink-0 text-[11px] tabular-nums text-secondary">
            成功率 {successRate.toFixed(1)}%
          </span>
        </div>
      )}
    </div>
  );
}

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function isSuccess(statusCode: number) {
  return statusCode >= 200 && statusCode < 300;
}

function outcomeTone(statusCode: number) {
  if (isSuccess(statusCode)) {
    return "bg-success";
  }
  if (statusCode >= 400 && statusCode < 500) {
    return "bg-warning";
  }
  return "bg-danger";
}
