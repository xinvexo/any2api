import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

export type RowActionTone = "neutral" | "danger";

export interface RowActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  tone?: RowActionTone;
  /** Quieter density for nested / embedded tables. */
  quiet?: boolean;
  children: ReactNode;
}

/**
 * Table/list row action control.
 * Keeps every inline action visually equal weight so none outshines the row content.
 */
export function RowActionButton({
  label,
  tone = "neutral",
  quiet = false,
  className,
  type = "button",
  children,
  ...props
}: RowActionButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      className={cn(
        "focus-ring inline-flex items-center font-medium tracking-tight transition-colors duration-150",
        "disabled:pointer-events-none disabled:opacity-40",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0",
        quiet
          ? "h-6 gap-1 rounded-[6px] px-1.5 text-[11px]"
          : "h-7 gap-1 rounded-[7px] px-2 text-[12px]",
        tone === "danger"
          ? "text-danger/75 hover:bg-danger/8 hover:text-danger"
          : "text-secondary hover:bg-surface-muted hover:text-primary",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
