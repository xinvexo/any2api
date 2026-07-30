import { useId, useRef, useState, type FocusEvent, type MouseEvent } from "react";

import type { RequestUsage, RequestUsageWindowSlot } from "../api/request-usage";
import { cn } from "@/shared/lib/cn";
import {
  FloatingPopover,
  anchorFromElement,
  resolveFloatingBounds,
  type FloatingPopoverAnchor,
} from "@/shared/ui/FloatingPopover";
import {
  formatSuccessRate,
  requestUsageSlotBarClass,
  requestUsageSlotTone,
  requestUsageSlotToneLabel,
  requestUsageSuccessRate,
} from "@/shared/ui/request-usage-tone";

interface HoverState {
  index: number;
  anchor: FloatingPopoverAnchor;
  bounds: DOMRect;
}

export function RequestUsageStats({
  label,
  usage,
  className,
}: {
  label: string;
  usage: RequestUsage;
  className?: string;
}) {
  const tooltipId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<HoverState | null>(null);
  const active = hover === null ? null : usage.windowSlots[hover.index];
  const outcomeSummary = usage.windowSlots
    .filter((slot) => slot.totalRequests > 0)
    .map((slot) => requestUsageSlotToneLabel(requestUsageSlotTone(slot)))
    .join("、");

  function openSlot(target: HTMLButtonElement, index: number) {
    setHover({
      index,
      anchor: anchorFromElement(target, "top"),
      bounds: resolveFloatingBounds(target),
    });
  }

  function onSlotEnter(event: MouseEvent<HTMLButtonElement>, index: number) {
    openSlot(event.currentTarget, index);
  }

  function onSlotFocus(event: FocusEvent<HTMLButtonElement>, index: number) {
    openSlot(event.currentTarget, index);
  }

  return (
    <div
      ref={rootRef}
      className={cn("flex min-w-0 max-w-full items-center gap-2.5", className)}
    >
      <div className="flex shrink-0 items-center gap-x-2 text-[11px] tabular-nums">
        <span className="font-medium text-[#1fa382] dark:text-[#24c19a]">
          成功 {formatCount(usage.successfulRequests)}
        </span>
        <span className="font-medium text-[#e11d48] dark:text-[#fb7185]">
          失败 {formatCount(usage.failedRequests)}
        </span>
      </div>

      <div
        className="flex h-[14px] w-full min-w-[9rem] max-w-[16rem] flex-1 items-stretch gap-[2px]"
        role="group"
        aria-label={`${label} 近 1 小时，每格 ${usage.windowMinutes} 分钟：${outcomeSummary || "暂无调用"}`}
        onMouseLeave={() => setHover(null)}
      >
        {usage.windowSlots.map((slot, index) => {
          const tone = requestUsageSlotTone(slot);
          return (
            <button
              key={slot.startedAtMs}
              type="button"
              className={cn(
                "min-w-[2px] flex-1 rounded-[2.5px] transition-[filter] duration-100",
                "focus-ring outline-none",
                requestUsageSlotBarClass(tone),
                hover?.index === index &&
                  "brightness-[1.08] saturate-[1.12]",
              )}
              aria-describedby={hover?.index === index ? tooltipId : undefined}
              aria-label={slotAriaLabel(slot, usage.windowMinutes, tone)}
              onMouseEnter={(event) => onSlotEnter(event, index)}
              onFocus={(event) => onSlotFocus(event, index)}
              onBlur={() => setHover(null)}
            />
          );
        })}
      </div>

      <FloatingPopover
        open={active !== null && hover !== null}
        anchor={hover?.anchor ?? null}
        bounds={hover?.bounds ?? null}
        id={tooltipId}
      >
        {active ? (
          <SlotTooltip slot={active} windowMinutes={usage.windowMinutes} />
        ) : null}
      </FloatingPopover>
    </div>
  );
}

function SlotTooltip({
  slot,
  windowMinutes,
}: {
  slot: RequestUsageWindowSlot;
  windowMinutes: number;
}) {
  const tone = requestUsageSlotTone(slot);
  const rate = requestUsageSuccessRate(slot);
  return (
    <>
      <p className="whitespace-nowrap tabular-nums text-secondary">
        {formatClock(slot.startedAtMs)}–{formatClock(slot.startedAtMs + windowMinutes * 60_000)}
        <span className="mx-1 text-tertiary">·</span>
        <span className={toneTextClass(tone)}>{requestUsageSlotToneLabel(tone)}</span>
      </p>
      <p className="mt-0.5 whitespace-nowrap tabular-nums">
        <span className="text-[#1fa382] dark:text-[#24c19a]">
          成功 {formatCount(slot.successfulRequests)}
        </span>
        <span className="mx-1 text-tertiary">·</span>
        <span className="text-[#e11d48] dark:text-[#fb7185]">
          失败 {formatCount(slot.failedRequests)}
        </span>
        {rate !== null ? (
          <>
            <span className="mx-1 text-tertiary">·</span>
            <span className="text-secondary">成功率 {formatSuccessRate(rate)}</span>
          </>
        ) : null}
      </p>
    </>
  );
}

function slotAriaLabel(
  slot: RequestUsageWindowSlot,
  windowMinutes: number,
  tone: ReturnType<typeof requestUsageSlotTone>,
) {
  const start = formatClock(slot.startedAtMs);
  const end = formatClock(slot.startedAtMs + windowMinutes * 60_000);
  const rate = requestUsageSuccessRate(slot);
  const rateText = rate === null ? "无调用" : `成功率 ${formatSuccessRate(rate)}`;
  return `${start} 至 ${end}，${requestUsageSlotToneLabel(tone)}，成功 ${slot.successfulRequests}，失败 ${slot.failedRequests}，${rateText}`;
}

function toneTextClass(tone: ReturnType<typeof requestUsageSlotTone>) {
  switch (tone) {
    case "empty":
      return "text-tertiary";
    case "ok":
      return "text-[#1fa382] dark:text-[#24c19a]";
    case "degraded":
      return "text-[#d97706] dark:text-[#fbbf24]";
    case "down":
      return "text-[#e11d48] dark:text-[#fb7185]";
  }
}

function formatCount(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value);
}

function formatClock(ms: number) {
  return new Date(ms).toLocaleString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}
