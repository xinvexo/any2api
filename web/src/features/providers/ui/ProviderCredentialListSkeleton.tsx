import { Skeleton } from "@/shared/ui/Skeleton";

export function ProviderCredentialListSkeleton() {
  return (
    <div
      className="min-h-[109px] min-w-0 overflow-hidden sm:min-h-[51px]"
      role="status"
      aria-label="正在读取 API Key 配置"
      aria-busy="true"
    >
      <span className="sr-only">正在读取 API Key 配置</span>
      <div className="min-w-0 max-w-full overflow-hidden rounded-[10px] bg-surface/80 px-2 py-2 sm:rounded-none sm:bg-transparent sm:px-0 sm:py-1.5">
        <div className="flex min-w-0 max-w-full flex-col gap-2 sm:flex-row sm:items-center sm:gap-3">
          <div className="min-w-0 flex-1 space-y-1 overflow-hidden">
            <div className="flex items-center gap-1.5">
              <Skeleton className="h-[18px] w-28 max-w-[45%]" />
              <Skeleton className="h-[18px] w-12" />
              <Skeleton className="h-[18px] w-8" />
            </div>
            <div className="flex items-center gap-2.5">
              <Skeleton className="h-[17px] w-24 max-w-[36%]" />
              <Skeleton className="h-[17px] w-20" />
              <Skeleton className="h-[17px] w-14" />
            </div>
          </div>

          <div className="flex w-full min-w-0 items-center gap-2.5 sm:w-72 sm:shrink-0">
            <Skeleton className="h-2.5 w-12 shrink-0" />
            <Skeleton className="h-2.5 w-12 shrink-0" />
            <Skeleton className="h-[14px] min-w-[9rem] max-w-[16rem] flex-1" />
          </div>

          <div className="flex min-w-0 items-center justify-end gap-0.5 sm:shrink-0">
            {Array.from({ length: 4 }, (_, index) => (
              <Skeleton key={index} className="size-6 rounded-[6px]" />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
