import { Skeleton } from "@/shared/ui/Skeleton";

const DETAIL_WIDTHS = ["w-24", "w-20", "w-16", "w-20", "w-14", "w-12", "w-24", "w-32"];

export function RequestLogExpandedSkeleton({ failed }: { failed: boolean }) {
  const detailCount = failed ? 8 : 7;

  return (
    <div
      className="min-w-0 max-w-full space-y-3 overflow-x-clip"
      role="status"
      aria-label="正在读取请求日志详情"
      aria-busy="true"
    >
      <span className="sr-only">正在读取请求日志详情</span>
      <div className="grid min-w-0 grid-cols-2 gap-x-3 gap-y-2 sm:grid-cols-3 lg:grid-cols-4">
        {DETAIL_WIDTHS.slice(0, detailCount).map((width, index) => (
          <div key={index} className="min-w-0 space-y-0.5">
            <Skeleton className="h-4 w-12" />
            <Skeleton
              className={`${index === 0 || (failed && index === 7) ? "h-8 sm:h-4" : "h-4"} ${width} max-w-full`}
            />
          </div>
        ))}
      </div>

      {failed ? (
        <div className="space-y-1.5">
          <Skeleton className="h-2.5 w-20" />
          <div className="flex min-h-8 items-center gap-2 rounded-[10px] bg-surface/80 px-2.5 py-1.5">
            <Skeleton className="h-3 w-5" />
            <Skeleton className="h-3 w-16" />
            <Skeleton className="h-3 w-12" />
            <Skeleton className="h-4 w-24 rounded-full" />
          </div>
        </div>
      ) : null}
    </div>
  );
}
