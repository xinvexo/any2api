import type { RequestSpeedTier } from "../api/request-log-contracts";
import { cn } from "@/shared/lib/cn";

interface RequestModeBadgesProps {
  isStream: boolean | null;
  requestedSpeedTier: RequestSpeedTier | null;
  effectiveSpeedTier: RequestSpeedTier | null;
}

export function RequestModeBadges({
  isStream,
  requestedSpeedTier,
  effectiveSpeedTier,
}: RequestModeBadgesProps) {
  const confirmedFast = effectiveSpeedTier === "fast";
  const unconfirmedFast = effectiveSpeedTier === null && requestedSpeedTier === "fast";
  if (isStream === null && !confirmedFast && !unconfirmedFast) {
    return null;
  }

  return (
    <span className="flex shrink-0 items-center gap-1">
      {isStream === null ? null : (
        <span
          aria-label={`请求模式：${isStream ? "流式" : "非流式"}`}
          className="inline-flex shrink-0 rounded-full bg-surface-muted px-1.5 py-px text-[10px] font-medium leading-4 text-secondary"
        >
          {isStream ? "流" : "非流"}
        </span>
      )}
      {confirmedFast || unconfirmedFast ? (
        <span
          aria-label={confirmedFast ? "Fast 模式" : "请求 Fast，上游尚未确认"}
          title={confirmedFast ? "Fast 模式" : "请求 Fast，上游尚未确认"}
          className={cn(
            "inline-flex shrink-0 rounded-full px-1.5 py-px text-[10px] font-medium leading-4 text-accent-copy",
            confirmedFast
              ? "bg-accent/12"
              : "bg-accent/[0.06] ring-1 ring-inset ring-accent/20",
          )}
        >
          Fast
        </span>
      ) : null}
    </span>
  );
}
