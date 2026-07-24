import { useId, useRef, useState, type FocusEvent, type MouseEvent } from "react";

import type {
  UpstreamRequestUsage,
  UpstreamRequestWindowSlot,
} from "../api/upstream-request-usage";
import { cn } from "@/shared/lib/cn";
import {
  FloatingPopover,
  anchorFromElement,
  resolveFloatingBounds,
  type FloatingPopoverAnchor,
} from "@/shared/ui/FloatingPopover";

interface HoverState {
  index: number;
  anchor: FloatingPopoverAnchor;
  bounds: DOMRect;
}

export function RequestUsageStats({
  label,
  usage,
}: {
  label: string;
  usage: UpstreamRequestUsage;
}) {
  const tooltipId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<HoverState | null>(null);
  const active = hover === null ? null : usage.windowSlots[hover.index];
  const outcomeSummary = usage.windowSlots
    .filter((slot) => slot.totalRequests > 0)
    .map((slot) => slotToneLabel(slot))
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
    <div ref={rootRef} className="flex min-w-0 max-w-full items-center gap-2">
      <div className="flex shrink-0 items-center gap-x-2 text-[11px] tabular-nums">
        <span className="font-medium text-success">
          成功 {formatCount(usage.successfulRequests)}
        </span>
        <span className="font-medium text-danger">
          失败 {formatCount(usage.failedRequests)}
        </span>
      </div>

      <div
        className="flex h-3.5 w-full min-w-[7.5rem] max-w-[12rem] flex-1 items-stretch gap-px"
        role="img"
        aria-label={`${label} 近 1 小时，每格 ${usage.windowMinutes} 分钟：${outcomeSummary || "暂无调用"}`}
        onMouseLeave={() => setHover(null)}
      >
        {usage.windowSlots.map((slot, index) => (
          <button
            key={slot.startedAtMs}
            type="button"
            className={cn(
              "min-w-[2px] flex-1 rounded-[2px] transition-[transform,filter,box-shadow] duration-100",
              "focus-ring outline-none",
              slotTone(slot),
              hover?.index === index &&
                "relative z-[1] scale-y-125 shadow-[0_0_0_1px_rgb(0_0_0/18%)] brightness-95",
            )}
            aria-describedby={hover?.index === index ? tooltipId : undefined}
            aria-label={slotAriaLabel(slot, usage.windowMinutes)}
            onMouseEnter={(event) => onSlotEnter(event, index)}
            onFocus={(event) => onSlotFocus(event, index)}
            onBlur={() => setHover(null)}
          />
        ))}
      </div>

      <FloatingPopover
        open={active !== null && hover !== null}
        anchor={hover?.anchor ?? null}
        bounds={hover?.bounds ?? null}
        id={tooltipId}
      >
        {active ? (
          <>
            <p className="whitespace-nowrap tabular-nums text-secondary">
              {formatClock(active.startedAtMs)}–
              {formatClock(active.startedAtMs + usage.windowMinutes * 60_000)}
            </p>
            <p className="mt-0.5 whitespace-nowrap tabular-nums">
              <span className="text-success">
                成功 {formatCount(active.successfulRequests)}
              </span>
              <span className="mx-1 text-tertiary">·</span>
              <span className="text-danger">
                失败 {formatCount(active.failedRequests)}
              </span>
            </p>
          </>
        ) : null}
      </FloatingPopover>
    </div>
  );
}

function slotTone(slot: UpstreamRequestWindowSlot) {
  if (slot.totalRequests === 0) {
    // Slightly stronger than surface-muted so bars stay visible on muted card chrome.
    return "bg-black/[0.08] dark:bg-white/[0.12]";
  }
  if (slot.failedRequests > 0) {
    return "bg-danger/85";
  }
  return "bg-success/85";
}

function slotToneLabel(slot: UpstreamRequestWindowSlot) {
  if (slot.failedRequests > 0) {
    return "失败";
  }
  return "成功";
}

function slotAriaLabel(slot: UpstreamRequestWindowSlot, windowMinutes: number) {
  const start = formatClock(slot.startedAtMs);
  const end = formatClock(slot.startedAtMs + windowMinutes * 60_000);
  return `${start} 至 ${end}，成功 ${slot.successfulRequests}，失败 ${slot.failedRequests}`;
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
