import { useId, useState, type FocusEvent, type MouseEvent } from "react";

import type {
  OAuthQuotaEstimate as Estimate,
  OAuthQuotaRateCard,
} from "../api/oauth-quota-contracts";
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
          aria-label="本地额度统计：暂无"
        >
          暂无
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
  return (
    <>
      <span
        {...triggerProps}
        className="focus-ring shrink-0 cursor-help rounded-[3px] border-b border-dotted border-current text-tertiary outline-none"
        aria-label={`本地额度统计：已用 ${usedUsd}，总量 ${capacityUsd}`}
      >
        {usedUsd}/{capacityUsd}
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
  const capacity = estimate.estimatedCapacityCredits;
  const used = estimate.estimatedUsedCredits;
  const remaining = estimate.estimatedRemainingCredits;

  return (
    <div className="w-[18rem] max-w-[calc(100vw-2rem)]">
      <div className="text-secondary">本地额度统计</div>
      {capacity === null || used === null || remaining === null ? (
        <p className="mt-1 text-secondary">暂无</p>
      ) : (
        <>
          <p className="mt-1 flex items-baseline justify-between gap-3 tabular-nums">
            <span className="text-[13px] font-semibold tracking-tight text-primary">
              {formatEstimateValue(used, rateCard)}/{formatEstimateValue(capacity, rateCard)}
            </span>
            <span className="shrink-0 text-[10px] text-tertiary">
              {rateCard ? "USD 等值" : "Credits"}
            </span>
          </p>
          <p className="mt-0.5 tabular-nums text-secondary">
            已用 {formatEstimateValue(used, rateCard)} · 剩余 {formatEstimateValue(remaining, rateCard)} · 总量 {formatEstimateValue(capacity, rateCard)}
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            Credits {formatCredits(used)} / {formatCredits(capacity)}{rateCard ? ` · ${rateCard.creditsPerUsd} Credits = $1` : ""}
          </p>
          <p className="text-[10px] tabular-nums text-tertiary">
            累计区间 {estimate.completedIntervalCount}
          </p>
        </>
      )}
    </div>
  );
}

function creditsToUsd(credits: number, creditsPerUsd: number) {
  return credits / creditsPerUsd;
}

function formatEstimateValue(value: number, rateCard: OAuthQuotaRateCard | null) {
  return rateCard
    ? formatEstimatedUsd(creditsToUsd(value, rateCard.creditsPerUsd))
    : `${formatCredits(value)} Credits`;
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
