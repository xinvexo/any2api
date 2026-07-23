import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

export type IconButtonTone = "neutral" | "danger";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  tone?: IconButtonTone;
  size?: "sm" | "md";
  children: ReactNode;
}

/** Square icon-only control for toolbars, drawers, and token actions. */
export function IconButton({
  label,
  tone = "neutral",
  size = "md",
  className,
  type = "button",
  children,
  ...props
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      className={cn(
        "focus-ring inline-flex shrink-0 items-center justify-center transition-colors duration-150",
        "disabled:pointer-events-none disabled:opacity-40",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0",
        size === "sm" ? "size-6 rounded-[6px]" : "size-8 rounded-[8px]",
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
