export function PageLoadingFallback() {
  return (
    <div
      className="flex min-h-56 flex-col items-center justify-center gap-3 text-sm text-secondary"
      role="status"
      aria-label="正在加载页面"
      aria-live="polite"
    >
      <span className="size-5 animate-pulse rounded-full bg-accent/70" aria-hidden="true" />
      <span>正在加载页面</span>
    </div>
  );
}
