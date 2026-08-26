import type { RequestUsageWindowSlot } from "../api/request-usage";

/**
 * Slot health inspired by status.openai.com:
 * green = operational, yellow = degraded/partial, red = outage.
 * Colored by success rate inside the window — not binary any-failure→red.
 */
export type RequestUsageSlotTone = "empty" | "ok" | "degraded" | "down";

const OK_SUCCESS_RATE_MINIMUM = 0.9;

export function requestUsageSlotTone(slot: RequestUsageWindowSlot): RequestUsageSlotTone {
  if (slot.totalRequests === 0) {
    return "empty";
  }
  const rate = slot.successfulRequests / slot.totalRequests;
  if (rate >= OK_SUCCESS_RATE_MINIMUM) {
    return "ok";
  }
  if (slot.successfulRequests > 0) {
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

export function requestUsageSlotBarClass(tone: RequestUsageSlotTone): string {
  switch (tone) {
    case "empty":
      return "bg-request-usage-empty";
    case "ok":
      return "bg-request-usage-ok";
    case "degraded":
      return "bg-request-usage-degraded";
    case "down":
      return "bg-request-usage-down";
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
