export function RequestAttemptMarker({ attemptCount }: { attemptCount: number }) {
  if (attemptCount <= 1) {
    return null;
  }

  return (
    <span
      role="img"
      aria-label={`共 ${attemptCount} 次上游尝试`}
      className="pointer-events-none absolute inset-1 z-0 rounded-[8px]"
      style={{
        boxShadow:
          "inset 1px 0 0 color-mix(in srgb, var(--danger) 34%, transparent)",
      }}
    />
  );
}
