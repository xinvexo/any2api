import type {
  OAuthQuotaUsdEstimate as Estimate,
} from "../api/oauth-quota-contracts";

export function QuotaUsdEstimate({ estimate }: { estimate: Estimate }) {
  const used = formatEstimatedUsd(estimate.estimatedUsedUsd);
  const capacity = formatEstimatedUsd(estimate.estimatedCapacityUsd);
  const remaining = formatEstimatedUsd(estimate.estimatedRemainingUsd);
  const sampleCost = formatEstimatedUsd(estimate.sampleCostUsd);
  const sampleDuration = formatSampleDuration(
    estimate.sampleEndedAt - estimate.sampleStartedAt,
  );
  const details = [
    "本机观测估算（非上游余额）",
    `已用 ${used} · 剩余 ${remaining} · 总量 ${capacity}`,
    `样本 ${sampleCost} / Δ${formatPercentDelta(estimate.sampleUsedPercentDelta)} · ${sampleDuration}`,
    `${formatSampleTime(estimate.sampleStartedAt)} → ${formatSampleTime(estimate.sampleEndedAt)}`,
    `费率卡 ${estimate.pricingBasis}`,
  ].join("\n");
  return (
    <span
      className="shrink-0 text-tertiary"
      aria-label={`本机观测估算：已用 ${used}，总量 ${capacity}`}
      title={details}
    >
      {used}/{capacity}
    </span>
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
  if (seconds < 60) return `${Math.max(0, Math.round(seconds))} 秒`;
  if (seconds < 3_600) return `${Math.round(seconds / 60)} 分钟`;
  return `${Math.round(seconds / 3_600)} 小时`;
}

function formatSampleTime(value: number) {
  return new Date(value * 1_000).toLocaleString();
}
