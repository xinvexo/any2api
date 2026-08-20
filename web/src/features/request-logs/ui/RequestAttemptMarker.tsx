export function RequestAttemptMarker({ attemptCount }: { attemptCount: number }) {
  if (attemptCount <= 1) {
    return null;
  }

  return (
    <span
      role="img"
      aria-label={`共 ${attemptCount} 次上游尝试`}
      className="pointer-events-none absolute inset-y-1 left-1 z-0 w-4 rounded-[999px] border-2 border-l-danger/60 border-t-danger/60 border-b-danger/60 border-r-transparent"
    />
  );
}
