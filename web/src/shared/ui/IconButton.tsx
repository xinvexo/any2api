import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cn } from "@/shared/lib/cn";

type IconButtonTone = "neutral" | "danger";

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  tone?: IconButtonTone;
  size?: "sm" | "md";
  /** Transparent at rest for controls embedded inside a surface. */
  quiet?: boolean;
  children: ReactNode;
}

/** Square icon-only control for toolbars, drawers, and token actions. */
export function IconButton({
  label,
  tone = "neutral",
  size = "md",
  quiet = false,
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
        quiet
          ? tone === "danger"
            ? "bg-transparent text-danger hover:bg-danger/10 active:bg-danger/14"
            : "bg-transparent text-secondary hover:bg-control-hover hover:text-primary active:bg-control-active"
          : tone === "danger"
            ? "bg-danger/10 text-danger hover:bg-danger/14 active:bg-danger/18"
            : "bg-control text-secondary hover:bg-control-hover hover:text-primary active:bg-control-active",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
}
