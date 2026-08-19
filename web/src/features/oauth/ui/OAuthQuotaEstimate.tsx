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

  if (used === null) {
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

  const usedDisplay = formatEstimateValue(used, rateCard);
  const capacityDisplay = capacity === null
    ? "暂无"
    : formatEstimateValue(capacity, rateCard);
  return (
    <>
      <span
        {...triggerProps}
        className="focus-ring shrink-0 cursor-help rounded-[3px] border-b border-dotted border-current text-tertiary outline-none"
        aria-label={`本地额度统计：已用 ${usedDisplay}，总量 ${capacityDisplay}`}
      >
        {usedDisplay}/{capacityDisplay}
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
      {used === null ? (
        <p className="mt-1 text-secondary">暂无</p>
      ) : (
        <>
          <p className="mt-1 flex items-baseline justify-between gap-3 tabular-nums">
            <span className="text-[13px] font-semibold tracking-tight text-primary">
              {formatEstimateValue(used, rateCard)}/{capacity === null ? "暂无" : formatEstimateValue(capacity, rateCard)}
            </span>
            <span className="shrink-0 text-[10px] text-tertiary">
              {rateCard ? "USD 等值" : "Credits"}
            </span>
          </p>
          {capacity === null ? (
            <p className="mt-0.5 tabular-nums text-secondary">
              本地已用 {formatEstimateValue(used, rateCard)} · 总量暂无
            </p>
          ) : (
            <p className="mt-0.5 tabular-nums text-secondary">
              已用 {formatEstimateValue(used, rateCard)} · 剩余 {remaining === null ? "暂无" : formatEstimateValue(remaining, rateCard)} · 总量 {formatEstimateValue(capacity, rateCard)}
            </p>
          )}
          <p className="text-[10px] tabular-nums text-tertiary">
            Credits {formatCredits(used)} / {capacity === null ? "暂无" : formatCredits(capacity)}{rateCard ? ` · ${rateCard.creditsPerUsd} Credits = $1` : ""}
          </p>
          <p className="mt-1 text-[10px] text-tertiary">
            本地已用为当前官方周期 RequestLog 直接总和。
          </p>
          <p className="text-[10px] text-tertiary">
            {capacity === null
              ? "总量需整周期可比、官方使用率至少 2% 且本地已用为正，当前暂不推算。"
              : "总量按当前周期本地已用与官方使用率的比例推算至整周期。"}
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
