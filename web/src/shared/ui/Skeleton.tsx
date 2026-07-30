import { cn } from "@/shared/lib/cn";

export function Skeleton({ className }: { className?: string }) {
  return (
    <span
      aria-hidden="true"
      data-skeleton
      className={cn("block animate-pulse rounded-[5px] bg-strong", className)}
    />
  );
}
