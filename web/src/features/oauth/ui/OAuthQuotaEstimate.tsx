import { useId, useState, type FocusEvent, type MouseEvent } from "react";

import type { OAuthQuotaEstimate as Estimate } from "../api/oauth-quota-contracts";
import { cn } from "@/shared/lib/cn";
import {
  FloatingPopover,
  anchorFromElement,
  resolveFloatingBounds,
  type FloatingPopoverAnchor,
} from "@/shared/ui/FloatingPopover";

const CODEX_CREDITS_PER_USD_2026_08_11 = 25;

interface HoverState {
  anchor: FloatingPopoverAnchor;
  bounds: DOMRect;
}

export function QuotaEstimate({ estimate }: { estimate: Estimate }) {
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
          aria-label="本地额度估算：学习中"
        >
          学习中
        </span>
        <FloatingPopover
          open={hover !== null}
          anchor={hover?.anchor ?? null}
          bounds={hover?.bounds ?? null}
          id={tooltipId}
        >
          <QuotaTooltip estimate={estimate} />
        </FloatingPopover>
      </>
    );
  }

  const usedUsd = formatEstimatedUsd(creditsToUsd(used));
  const capacityUsd = formatEstimatedUsd(creditsToUsd(capacity));
  const prefix = isApproximate(estimate.confidence) ? "≈" : "";
  return (
    <>
      <span
        {...triggerProps}
        className="focus-ring shrink-0 cursor-help rounded-[3px] border-b border-dotted border-current text-tertiary outline-none"
        aria-label={`本地额度估算：已用 ${usedUsd}，总量 ${capacityUsd}，${confidenceLabel(estimate.confidence)}`}
      >
        {prefix}{usedUsd}/{capacityUsd}
      </span>
      <FloatingPopover
        open={hover !== null}
        anchor={hover?.anchor ?? null}
        bounds={hover?.bounds ?? null}
        id={tooltipId}
      >
        <QuotaTooltip estimate={estimate} />
      </FloatingPopover>
    </>
  );
}

function QuotaTooltip({ estimate }: { estimate: Estimate }) {
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
          置信度 {confidenceLabel(estimate.confidence)}
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
              {formatEstimatedUsd(creditsToUsd(used))}/{formatEstimatedUsd(creditsToUsd(capacity))}
            </span>
            <span className="shrink-0 text-[10px] text-tertiary">USD 等值</span>
          </p>
          <p className="mt-0.5 tabular-nums text-secondary">
            已用 {formatEstimatedUsd(creditsToUsd(used))} · 剩余 {formatEstimatedUsd(creditsToUsd(remaining))} · 总量 {formatEstimatedUsd(creditsToUsd(capacity))}
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            Credits {formatCredits(used)} / {formatCredits(capacity)} · 25 Credits = $1（仅展示换算）
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            容量样本 {estimate.sampleCount} · 本窗口期 {estimate.freshSampleCount}
          </p>
        </>
      )}

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
            未计价请求 {interval.unpricedRequestCount} 条，本区间未参与学习
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
    interval.prunedRequestLogs > 0 ? `日志清理 ${interval.prunedRequestLogs}` : null,
  ].filter(Boolean);
  return parts.length > 0 ? `遥测缺口：${parts.join(" · ")}` : null;
}

function isApproximate(value: Estimate["confidence"]) {
  return value === "degraded" || value === "inherited";
}

function confidenceLabel(value: Estimate["confidence"]) {
  switch (value) {
    case "unknown": return "未知";
    case "learning": return "学习中";
    case "stable": return "稳定";
    case "inherited": return "沿用上期";
    case "degraded": return "已降级";
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
    case "external_usage_suspected": return "疑似外部消费";
    case "outlier_rejected": return "异常样本已拒绝";
    case "invalid": return "区间无效";
  }
}

function intervalStatusTone(value: Estimate["latestInterval"]["status"]) {
  switch (value) {
    case "valid_sample": return "text-success";
    case "external_usage_suspected":
    case "outlier_rejected":
    case "telemetry_incomplete":
    case "unpriced_usage":
    case "invalid":
      return "text-warning";
    default:
      return "text-secondary";
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
  return new Date(value * 1_000).toLocaleString(undefined, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}
