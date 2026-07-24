import type { ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

/** Shared by 上游提供 / 认证文件 so route switches keep the same chrome geometry. */
export const KIND_SPLIT_GRID_CLASS =
  "grid grid-cols-1 gap-x-5 gap-y-3 sm:grid-cols-[13rem_minmax(0,1fr)] lg:grid-cols-[14rem_minmax(0,1fr)]";

interface KindSplitLayoutProps {
  /** Left toolbar slot (e.g. search). Empty still reserves desktop height/width. */
  toolbarStart?: ReactNode;
  toolbarEnd: ReactNode;
  kindNav: ReactNode;
  children: ReactNode;
  className?: string;
  "aria-busy"?: boolean | "true" | "false";
}

/**
 * Desktop:
 *   row1 col2 = toolbar (start + end)
 *   row2 col1 = kind nav, row2 col2 = content
 * Mobile: stack toolbar → kinds → content.
 */
export function KindSplitLayout({
  toolbarStart,
  toolbarEnd,
  kindNav,
  children,
  className,
  "aria-busy": ariaBusy,
}: KindSplitLayoutProps) {
  return (
    <div className={cn(KIND_SPLIT_GRID_CLASS, className)} aria-busy={ariaBusy}>
      <div className="flex min-h-8 flex-col gap-2.5 sm:col-start-2 sm:row-start-1 sm:flex-row sm:items-center sm:justify-between">
        <div
          className={cn(
            "relative min-w-0 flex-1 sm:flex sm:min-h-8 sm:max-w-sm sm:items-center",
            toolbarStart ? "flex min-h-8 items-center" : "hidden",
          )}
        >
          {toolbarStart}
        </div>
        <div className="flex min-h-8 shrink-0 items-center gap-1.5">{toolbarEnd}</div>
      </div>

      <div className="sm:col-start-1 sm:row-start-2">{kindNav}</div>
      <div className="min-w-0 sm:col-start-2 sm:row-start-2">{children}</div>
    </div>
  );
}
