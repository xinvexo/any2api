export function RequestAttemptMarker({ attemptCount }: { attemptCount: number }) {
  if (attemptCount <= 1) {
    return null;
  }

  return (
    <span
      role="img"
      aria-label={`共 ${attemptCount} 次上游尝试`}
      className="pointer-events-none absolute inset-y-0 left-0 z-0 w-8 rounded-l-[8px] rounded-r-full bg-gradient-to-r from-danger/20 via-danger/8 to-transparent"
    />
  );
}
