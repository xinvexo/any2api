import type { RequestUsageWindowSlot } from "../api/request-usage";

/**
 * Slot health inspired by status.openai.com:
 * green = operational, yellow = degraded/partial, red = outage.
 * Colored by success rate inside the window — not binary any-failure→red.
 */
export type RequestUsageSlotTone = "empty" | "ok" | "degraded" | "down";

export function requestUsageSlotTone(slot: RequestUsageWindowSlot): RequestUsageSlotTone {
  if (slot.totalRequests === 0) {
    return "empty";
  }
  const rate = slot.successfulRequests / slot.totalRequests;
  if (rate >= 1) {
    return "ok";
  }
  if (rate >= 0.5) {
    return "degraded";
  }
  return "down";
}

export function requestUsageSlotToneLabel(tone: RequestUsageSlotTone): string {
  switch (tone) {
    case "empty":
      return "无调用";
    case "ok":
      return "正常";
    case "degraded":
      return "降级";
    case "down":
      return "故障";
  }
}

/** Soft status-page fills (OpenAI / incident.io palette). */
export function requestUsageSlotBarClass(tone: RequestUsageSlotTone): string {
  switch (tone) {
    case "empty":
      return "bg-black/[0.06] dark:bg-white/[0.08]";
    case "ok":
      return "bg-[#24c19a] dark:bg-[#1fa382]";
    case "degraded":
      return "bg-[#fbbf24] dark:bg-[#f59e0b]";
    case "down":
      return "bg-[#f87171] dark:bg-[#ef4444]";
  }
}

export function requestUsageSuccessRate(slot: Pick<
  RequestUsageWindowSlot,
  "totalRequests" | "successfulRequests"
>): number | null {
  if (slot.totalRequests === 0) {
    return null;
  }
  return slot.successfulRequests / slot.totalRequests;
}

export function formatSuccessRate(rate: number | null): string {
  if (rate === null) {
    return "—";
  }
  return `${Math.round(rate * 100)}%`;
}
