import type { OAuthQuotaEstimate as Estimate } from "../api/oauth-quota-contracts";

const CODEX_CREDITS_PER_USD_2026_08_11 = 25;

export function QuotaEstimate({ estimate }: { estimate: Estimate }) {
  const used = estimate.estimatedUsedCredits;
  const capacity = estimate.estimatedCapacityCredits;
  if (used === null || capacity === null) {
    return (
      <span
        className="shrink-0 text-tertiary"
        title={details(estimate)}
      >
        学习中
      </span>
    );
  }
  const usedUsd = formatEstimatedUsd(creditsToUsd(used));
  const capacityUsd = formatEstimatedUsd(creditsToUsd(capacity));
  const prefix = estimate.confidence === "degraded" ? "≈" : "";
  return (
    <span
      className="shrink-0 cursor-help border-b border-dotted border-current text-tertiary"
      aria-label={`本地额度估算：已用 ${usedUsd}，总量 ${capacityUsd}，${confidenceLabel(estimate.confidence)}`}
      title={details(estimate)}
    >
      {prefix}{usedUsd}/{capacityUsd}
    </span>
  );
}

function details(estimate: Estimate) {
  const interval = estimate.latestInterval;
  const capacity = estimate.estimatedCapacityCredits;
  const used = estimate.estimatedUsedCredits;
  const remaining = estimate.estimatedRemainingCredits;
  const amounts = capacity === null || used === null || remaining === null
    ? ["容量尚未形成：需要两个可靠官方快照和完整的本地遥测区间"]
    : [
      `已用 ${formatEstimatedUsd(creditsToUsd(used))} · 剩余 ${formatEstimatedUsd(creditsToUsd(remaining))} · 总量 ${formatEstimatedUsd(creditsToUsd(capacity))}`,
      `Credits ${formatCredits(used)} / ${formatCredits(capacity)} · 25 Credits = $1（仅展示换算）`,
    ];
  const intervalRange = interval.startedAt === null
    ? `观测于 ${formatTime(interval.endedAt)}`
    : `${formatTime(interval.startedAt)} → ${formatTime(interval.endedAt)}`;
  const localCost = interval.localCostCredits === null
    ? null
    : `区间本地消费 ${formatCredits(interval.localCostCredits)} Credits`;
  const delta = interval.deltaUsedPercent === null
    ? null
    : `官方使用率变化 ${formatPercent(interval.deltaUsedPercent)}`;
  const losses = telemetryLosses(estimate);
  return [
    "本地观测估算（非上游余额）",
    ...amounts,
    `置信度 ${confidenceLabel(estimate.confidence)} · ${estimate.sampleCount} 个样本 · Epoch ${estimate.epoch}`,
    `最近区间 ${intervalStatusLabel(interval.status)} · ${intervalRange}`,
    [localCost, delta].filter(Boolean).join(" · "),
    interval.unpricedRequestCount > 0
      ? `未计价请求 ${interval.unpricedRequestCount} 条，本区间未参与学习`
      : null,
    losses,
    estimate.relativeMad === null
      ? null
      : `样本相对 MAD ${formatPercent(estimate.relativeMad * 100)}`,
    estimate.rateCards.length > 0 ? `费率卡 ${estimate.rateCards.join(", ")}` : null,
  ].filter(Boolean).join("\n");
}

function telemetryLosses(estimate: Estimate) {
  const interval = estimate.latestInterval;
  const parts = [
    interval.queueDroppedRequestLogs > 0 ? `队列丢失 ${interval.queueDroppedRequestLogs}` : null,
    interval.storageFailedRequestLogs > 0 ? `写入失败 ${interval.storageFailedRequestLogs}` : null,
    interval.prunedRequestLogs > 0 ? `日志清理 ${interval.prunedRequestLogs}` : null,
  ].filter(Boolean);
  return parts.length > 0 ? `遥测缺口：${parts.join(" · ")}` : null;
}

function confidenceLabel(value: Estimate["confidence"]) {
  switch (value) {
    case "unknown": return "未知";
    case "learning": return "学习中";
    case "stable": return "稳定";
    case "degraded": return "已降级";
  }
}

function intervalStatusLabel(value: Estimate["latestInterval"]["status"]) {
  switch (value) {
    case "awaiting_baseline": return "等待基线";
    case "no_change": return "变化不足，未采样";
    case "valid_sample": return "有效样本";
    case "reset_boundary": return "检测到额度重置";
    case "telemetry_incomplete": return "本地遥测不完整";
    case "unpriced_usage": return "存在未计价请求";
    case "external_usage_suspected": return "疑似外部消费";
    case "outlier_rejected": return "异常样本已拒绝";
    case "invalid": return "区间无效";
  }
}

function creditsToUsd(credits: number) {
  return credits / CODEX_CREDITS_PER_USD_2026_08_11;
}

function formatEstimatedUsd(value: number) {
  const maximumFractionDigits = value < 0.01 ? 4 : 2;
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits,
  }).format(value);
}

function formatCredits(value: number) {
  return new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value);
}

function formatPercent(value: number) {
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 }).format(value)}%`;
}

function formatTime(value: number) {
  return new Date(value * 1_000).toLocaleString();
}
