import type { ButtonHTMLAttributes } from "react";

import { cn } from "@/shared/lib/cn";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost";
}

const variants = {
  primary: "bg-accent text-on-accent hover:bg-accent-strong shadow-accent",
  secondary: "border border-subtle bg-surface text-primary hover:bg-surface-hover shadow-hairline",
  ghost: "text-secondary hover:bg-surface-hover hover:text-primary",
};

export function Button({ className, type = "button", variant = "secondary", ...props }: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "focus-ring inline-flex h-10 items-center justify-center gap-2 rounded-control px-4 text-sm font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-50",
        variants[variant],
        className,
      )}
      {...props}
    />
  );
}
