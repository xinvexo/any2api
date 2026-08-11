import type {
  OAuthQuotaUsdEstimate as Estimate,
} from "../api/oauth-quota-contracts";

export function QuotaUsdEstimate({ estimate }: { estimate: Estimate }) {
  const sampleDuration = formatSampleDuration(
    estimate.sampleEndedAt - estimate.sampleStartedAt,
  );
  return (
    <div className="mt-1.5 space-y-0.5 text-[10px] tabular-nums text-tertiary">
      <p>
        本机观测估算：已用 {formatEstimatedUsd(estimate.estimatedUsedUsd)}
        {" · "}剩余 {formatEstimatedUsd(estimate.estimatedRemainingUsd)}
        {" · "}总量 {formatEstimatedUsd(estimate.estimatedCapacityUsd)}
      </p>
      <p title={`费率卡：${estimate.pricingBasis}`}>
        样本 {formatEstimatedUsd(estimate.sampleCostUsd)} / Δ
        {formatPercentDelta(estimate.sampleUsedPercentDelta)}
        {sampleDuration ? ` · ${sampleDuration}` : ""}
        {" · 官方标准 API 价 · 非上游余额"}
      </p>
    </div>
  );
}

function formatEstimatedUsd(value: number) {
  const maximumFractionDigits = value < 0.01 ? 6 : value < 1 ? 4 : 2;
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits,
  }).format(value);
}

function formatPercentDelta(value: number) {
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value)}%`;
}

function formatSampleDuration(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) return null;
  if (seconds < 60) return `${Math.round(seconds)} 秒`;
  if (seconds < 3_600) return `${Math.round(seconds / 60)} 分钟`;
  return `${Math.round(seconds / 3_600)} 小时`;
}
