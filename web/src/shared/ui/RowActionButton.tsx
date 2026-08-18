import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

type RowActionTone = "neutral" | "success" | "danger";

export interface RowActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  tone?: RowActionTone;
  /** Quieter density for nested / embedded tables. */
  quiet?: boolean;
  children: ReactNode;
}

const toneClassName: Record<RowActionTone, string> = {
  neutral: "bg-control text-primary hover:bg-control-hover active:bg-control-active",
  success: "bg-success/10 text-success hover:bg-success/14 active:bg-success/18",
  danger: "bg-danger/10 text-danger hover:bg-danger/14 active:bg-danger/18",
};

/**
 * Table/list row action control with the same quiet fill as page commands.
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
        toneClassName[tone],
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
