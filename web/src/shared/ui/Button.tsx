import type { ButtonHTMLAttributes } from "react";

import { cn } from "@/shared/lib/cn";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "dangerSolid";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

/**
 * Restrained macOS-style push buttons.
 * Filled variants (primary / dangerSolid) always use white labels on solid color.
 */
const variants: Record<ButtonVariant, string> = {
  primary: [
    "ui-btn-fill bg-accent",
    "hover:bg-accent-strong active:brightness-[0.97]",
  ].join(" "),
  secondary: [
    "bg-surface-muted text-primary",
    "hover:bg-surface-hover active:bg-surface-hover",
  ].join(" "),
  ghost: [
    "bg-transparent text-secondary",
    "hover:bg-surface-muted hover:text-primary",
    "active:bg-surface-hover",
  ].join(" "),
  danger: "bg-transparent text-danger hover:bg-danger/10 active:bg-danger/14",
  dangerSolid: [
    "ui-btn-fill bg-danger",
    "hover:brightness-[0.96] active:brightness-[0.92]",
  ].join(" "),
};

/* ~28px height, 6px radius — system push button proportions */
const sizes: Record<ButtonSize, string> = {
  sm: "h-7 min-h-7 gap-1 rounded-[6px] px-2.5 text-[12px]",
  md: "h-7 min-h-7 gap-1 rounded-[6px] px-3 text-[13px]",
  lg: "h-8 min-h-8 gap-1.5 rounded-[7px] px-3.5 text-[13px]",
};

export function Button({
  className,
  type = "button",
  variant = "secondary",
  size = "md",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      data-variant={variant}
      className={cn(
        "focus-ring inline-flex w-auto shrink-0 items-center justify-center font-medium tracking-tight",
        "transition-[color,background-color,filter,opacity] duration-150",
        "disabled:pointer-events-none disabled:opacity-40",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg]:text-current",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
