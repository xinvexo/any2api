import type { ButtonHTMLAttributes } from "react";

import { cn } from "@/shared/lib/cn";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "dangerSolid";
}

const variants = {
  primary: "bg-accent text-on-accent hover:bg-accent-strong",
  secondary: "bg-surface-muted text-primary hover:bg-surface-hover",
  ghost: "bg-transparent text-secondary hover:bg-surface-muted hover:text-primary",
  danger: "bg-transparent text-danger hover:bg-danger/8",
  dangerSolid: "bg-danger text-on-danger shadow-none hover:brightness-95 active:brightness-90",
};

export function Button({
  className,
  type = "button",
  variant = "secondary",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "focus-ring inline-flex h-8 items-center justify-center gap-1.5 rounded-[8px] px-3 text-[12px] font-medium tracking-tight transition-colors duration-150",
        "disabled:cursor-not-allowed disabled:opacity-45",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
