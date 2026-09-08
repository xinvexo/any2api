import type { RequestSpeedTier } from "../api/request-log-contracts";

interface RequestModeBadgesProps {
  isStream: boolean | null;
  requestedSpeedTier: RequestSpeedTier | null;
  thinkingLevel?: string | null;
}

export function RequestModeBadges({
  isStream,
  requestedSpeedTier,
  thinkingLevel,
}: RequestModeBadgesProps) {
  if (isStream === null && requestedSpeedTier !== "fast" && !thinkingLevel) {
    return null;
  }

  return (
    <span className="flex min-w-0 items-center gap-1">
      <RequestStreamBadge isStream={isStream} />
      <RequestFastBadge requestedSpeedTier={requestedSpeedTier} />
      {thinkingLevel ? (
        <span className="truncate text-xs leading-4 text-secondary" aria-label={`思考深度：${thinkingLevel}`} title={thinkingLevel}>
          {thinkingLevel}
        </span>
      ) : null}
    </span>
  );
}

export function RequestStreamBadge({ isStream }: { isStream: boolean | null }) {
  if (isStream === null) {
    return null;
  }

  return (
    <span
      aria-label={`请求模式：${isStream ? "流式" : "非流式"}`}
      className={
        isStream
          ? "inline-flex shrink-0 rounded-full bg-accent/12 px-1.5 py-0.5 text-xs font-medium leading-4 text-accent-copy"
          : "inline-flex shrink-0 rounded-full bg-surface-muted px-1.5 py-0.5 text-xs font-medium leading-4 text-secondary"
      }
    >
      {isStream ? "流" : "非流"}
    </span>
  );
}

export function RequestFastBadge({
  requestedSpeedTier,
}: {
  requestedSpeedTier: RequestSpeedTier | null;
}) {
  if (requestedSpeedTier !== "fast") {
    return null;
  }

  return (
    <span
      aria-label="Fast 模式"
      title="Fast 模式"
      className="inline-flex shrink-0 rounded-full bg-accent/12 px-1.5 py-px text-xs font-medium leading-4 text-accent-copy"
    >
      Fast
    </span>
  );
}
