import { useId, useState, type FocusEvent, type MouseEvent } from "react";

import type {
  OAuthQuotaEstimate as Estimate,
  OAuthQuotaRateCard,
} from "../api/oauth-quota-contracts";
import { cn } from "@/shared/lib/cn";
import {
  FloatingPopover,
  anchorFromElement,
  resolveFloatingBounds,
  type FloatingPopoverAnchor,
} from "@/shared/ui/FloatingPopover";

interface HoverState {
  anchor: FloatingPopoverAnchor;
  bounds: DOMRect;
}

export function QuotaEstimate({
  estimate,
  rateCard,
}: {
  estimate: Estimate;
  rateCard: OAuthQuotaRateCard | null;
}) {
  const tooltipId = useId();
  const [hover, setHover] = useState<HoverState | null>(null);
  const used = estimate.estimatedUsedCredits;
  const capacity = estimate.estimatedCapacityCredits;

  function openTooltip(target: HTMLElement) {
    setHover({
      anchor: anchorFromElement(target, "top"),
      bounds: resolveFloatingBounds(target),
    });
  }

  function onMouseEnter(event: MouseEvent<HTMLSpanElement>) {
    openTooltip(event.currentTarget);
  }

  function onFocus(event: FocusEvent<HTMLSpanElement>) {
    openTooltip(event.currentTarget);
  }

  const triggerProps = {
    tabIndex: 0,
    "aria-describedby": hover ? tooltipId : undefined,
    onMouseEnter,
    onMouseLeave: () => setHover(null),
    onFocus,
    onBlur: () => setHover(null),
  };

  if (used === null || capacity === null) {
    return (
      <>
        <span
          {...triggerProps}
          className="focus-ring shrink-0 cursor-help rounded-[3px] text-tertiary outline-none"
          aria-label="本地额度估算：容量校准中"
        >
          容量校准中
        </span>
        <FloatingPopover
          open={hover !== null}
          anchor={hover?.anchor ?? null}
          bounds={hover?.bounds ?? null}
          id={tooltipId}
        >
          <QuotaTooltip estimate={estimate} rateCard={rateCard} />
        </FloatingPopover>
      </>
    );
  }

  const usedUsd = formatEstimateValue(used, rateCard);
  const capacityUsd = formatEstimateValue(capacity, rateCard);
  const prefix = isApproximate(estimate.confidence) ? "≈" : "";
  return (
    <>
      <span
        {...triggerProps}
        className="focus-ring shrink-0 cursor-help rounded-[3px] border-b border-dotted border-current text-tertiary outline-none"
        aria-label={`本地额度估算：已用 ${usedUsd}，总量 ${capacityUsd}，证据状态 ${confidenceLabel(estimate.confidence)}`}
      >
        {prefix}{usedUsd}/{capacityUsd}
      </span>
      <FloatingPopover
        open={hover !== null}
        anchor={hover?.anchor ?? null}
        bounds={hover?.bounds ?? null}
        id={tooltipId}
      >
        <QuotaTooltip estimate={estimate} rateCard={rateCard} />
      </FloatingPopover>
    </>
  );
}

function QuotaTooltip({
  estimate,
  rateCard,
}: {
  estimate: Estimate;
  rateCard: OAuthQuotaRateCard | null;
}) {
  const interval = estimate.latestInterval;
  const capacity = estimate.estimatedCapacityCredits;
  const used = estimate.estimatedUsedCredits;
  const remaining = estimate.estimatedRemainingCredits;
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

  return (
    <div className="w-[18rem] max-w-[calc(100vw-2rem)]">
      <div className="flex items-center justify-between gap-3">
        <span className="text-secondary">本地观测估算</span>
        <span
          className={cn(
            "shrink-0 rounded-full bg-surface-muted px-1.5 py-px text-[10px] font-medium",
            confidenceTone(estimate.confidence),
          )}
        >
          证据状态 {confidenceLabel(estimate.confidence)}
        </span>
      </div>

      {capacity === null || used === null || remaining === null ? (
        <p className="mt-1 text-secondary">
          容量尚未形成：需要两个可靠官方快照和完整的本地遥测区间
        </p>
      ) : (
        <>
          <p className="mt-1 flex items-baseline justify-between gap-3 tabular-nums">
            <span className="text-[13px] font-semibold tracking-tight text-primary">
              {isApproximate(estimate.confidence) ? "≈" : ""}
              {formatEstimateValue(used, rateCard)}/{formatEstimateValue(capacity, rateCard)}
            </span>
            <span className="shrink-0 text-[10px] text-tertiary">{rateCard ? "USD 等值" : "Credits"}</span>
          </p>
          <p className="mt-0.5 tabular-nums text-secondary">
            已用 {formatEstimateValue(used, rateCard)} · 剩余 {formatEstimateValue(remaining, rateCard)} · 总量 {formatEstimateValue(capacity, rateCard)}
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            Credits {formatCredits(used)} / {formatCredits(capacity)}{rateCard ? ` · ${rateCard.creditsPerUsd} Credits = $1（仅展示换算）` : ""}
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            容量样本 {estimate.sampleCount} · 本窗口期 {estimate.freshSampleCount}
          </p>
        </>
      )}

      <p className="mt-1 text-[10px] text-tertiary">
        {evidenceExplanation(estimate)}
      </p>

      <div className="mt-1.5 space-y-0.5 border-t border-subtle/60 pt-1.5 text-[10px] text-secondary">
        <p>
          最近区间 <span className={intervalStatusTone(interval.status)}>{intervalStatusLabel(interval.status)}</span>
        </p>
        <p className="tabular-nums text-tertiary">{intervalRange}</p>
        {localCost || delta ? (
          <p className="tabular-nums text-tertiary">
            {[localCost, delta].filter(Boolean).join(" · ")}
          </p>
        ) : null}
        {interval.unpricedRequestCount > 0 ? (
          <p className="text-warning">
            未计价请求 {interval.unpricedRequestCount} 条，本区间未参与校准
          </p>
        ) : null}
        {losses ? <p className="text-warning">{losses}</p> : null}
      </div>
      {estimate.rateCards.length > 0 ? (
        <p className="mt-1 truncate text-[10px] text-tertiary">
          费率卡 {estimate.rateCards.join(", ")}
        </p>
      ) : null}
    </div>
  );
}

function telemetryLosses(estimate: Estimate) {
  const interval = estimate.latestInterval;
  const parts = [
    interval.queueDroppedRequestLogs > 0 ? `队列丢失 ${interval.queueDroppedRequestLogs}` : null,
    interval.storageFailedRequestLogs > 0 ? `写入失败 ${interval.storageFailedRequestLogs}` : null,
    interval.intervalPruned ? "日志清理删除了区间数据" : null,
  ].filter(Boolean);
  return parts.length > 0 ? `遥测缺口：${parts.join(" · ")}` : null;
}

function isApproximate(value: Estimate["confidence"]) {
  return value === "degraded";
}

function confidenceLabel(value: Estimate["confidence"]) {
  switch (value) {
    case "unknown": return "无样本";
    case "learning": return "容量校准中";
    case "stable": return "稳定";
    case "degraded": return "不完整";
  }
}

function confidenceTone(value: Estimate["confidence"]) {
  return value === "degraded" ? "text-warning" : "text-secondary";
}

function intervalStatusLabel(value: Estimate["latestInterval"]["status"]) {
  switch (value) {
    case "awaiting_baseline": return "等待基线";
    case "no_change": return "变化不足，未采样";
    case "accumulating": return "累计观测中";
    case "valid_sample": return "有效样本";
    case "reset_boundary": return "检测到额度重置";
    case "telemetry_incomplete": return "本地遥测不完整";
    case "unpriced_usage": return "存在未计价请求";
    case "invalid": return "区间无效";
  }
}

function intervalStatusTone(value: Estimate["latestInterval"]["status"]) {
  switch (value) {
    case "valid_sample": return "text-success";
    case "telemetry_incomplete":
    case "unpriced_usage":
    case "invalid":
      return "text-warning";
    default:
      return "text-secondary";
  }
}

function creditsToUsd(credits: number, creditsPerUsd: number) {
  return credits / creditsPerUsd;
}

function formatEstimateValue(value: number, rateCard: OAuthQuotaRateCard | null) {
  return rateCard
    ? formatEstimatedUsd(creditsToUsd(value, rateCard.creditsPerUsd))
    : `${formatCredits(value)} Credits`;
}

function evidenceExplanation(estimate: Estimate) {
  switch (estimate.confidence) {
    case "unknown":
      return "尚无容量样本；这是证据状态，不是概率。";
    case "learning":
      if (estimate.sampleCount < 3) {
        return `已有 ${estimate.sampleCount} 个容量样本，至少 3 个才标记稳定；不是概率。`;
      }
      return `样本一致性仍不足${formatSampleDifference(estimate.relativeMad)}；样本差异不高于 20% 才标记稳定。`;
    case "stable":
      return `${estimate.sampleCount} 个样本且一致性达标${formatSampleDifference(estimate.relativeMad)}；这是证据状态，不是概率。`;
    case "degraded":
      return "最近区间存在未计价、无效观测或遥测缺口；现有估算仅供参考。";
  }
}

function formatSampleDifference(value: number | null) {
  return value === null ? "" : `（样本差异 ${formatPercent(value * 100)}）`;
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
  return new Date(value * 1_000).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}
