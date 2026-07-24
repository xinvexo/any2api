import type { ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

/**
 * Shared by 上游提供 / 认证文件 so route switches keep the same chrome geometry.
 *
 * Mobile column order: toolbarStart → kindNav → toolbarEnd → content
 * Desktop grid:
 *   row1: [empty] | start … end
 *   row2: kind    | content
 */
export const KIND_SPLIT_GRID_CLASS = cn(
  "grid grid-cols-1 gap-2.5",
  "[grid-template-areas:'start'_'kind'_'end'_'content']",
  "sm:grid-cols-[13rem_minmax(0,1fr)_auto] sm:gap-x-5 sm:gap-y-3",
  "sm:[grid-template-areas:'._start_end'_'kind_content_content']",
  "lg:grid-cols-[14rem_minmax(0,1fr)_auto]",
);

interface KindSplitLayoutProps {
  /** Left toolbar slot (e.g. search). Omitted when empty. */
  toolbarStart?: ReactNode;
  toolbarEnd: ReactNode;
  kindNav: ReactNode;
  children: ReactNode;
  className?: string;
  "aria-busy"?: boolean | "true" | "false";
}

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
      <div
        className={cn(
          "min-w-0 [grid-area:start]",
          toolbarStart
            ? "relative flex min-h-8 items-center sm:max-w-sm"
            : "hidden sm:block",
        )}
      >
        {toolbarStart}
      </div>

      <div className="min-w-0 [grid-area:kind]">{kindNav}</div>

      <div className="flex min-h-8 min-w-0 flex-wrap items-center justify-end gap-1.5 [grid-area:end] sm:self-center">
        {toolbarEnd}
      </div>

      <div className="min-w-0 [grid-area:content]">{children}</div>
    </div>
  );
}
