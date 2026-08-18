import { cn } from "@/shared/lib/cn";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger" | "dangerSolid";
export type ButtonSize = "sm" | "md" | "lg";

const variants: Record<ButtonVariant, string> = {
  primary: [
    "bg-control-strong text-on-control-strong",
    "hover:bg-control-strong-hover active:bg-control-strong-active",
  ].join(" "),
  secondary: [
    "bg-control text-primary",
    "hover:bg-control-hover active:bg-control-active",
  ].join(" "),
  ghost: [
    "bg-control text-primary",
    "hover:bg-control-hover active:bg-control-active",
  ].join(" "),
  danger: "bg-danger/10 text-danger hover:bg-danger/14 active:bg-danger/18",
  dangerSolid: [
    "bg-danger text-on-danger",
    "hover:brightness-[0.96] active:brightness-[0.92]",
  ].join(" "),
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 min-h-7 gap-1 rounded-[6px] px-2.5 text-[12px]",
  md: "h-7 min-h-7 gap-1 rounded-[6px] px-3 text-[13px]",
  lg: "h-8 min-h-8 gap-1.5 rounded-[7px] px-3.5 text-[13px]",
};

interface ButtonClassNameOptions {
  variant?: ButtonVariant;
  size?: ButtonSize;
  className?: string;
}

/** Lets navigation links use the same command anatomy as real buttons. */
export function buttonClassName({
  variant = "secondary",
  size = "md",
  className,
}: ButtonClassNameOptions = {}) {
  return cn(
    "focus-ring inline-flex w-auto shrink-0 items-center justify-center font-medium tracking-tight",
    "transition-[color,background-color,filter,opacity] duration-150",
    "disabled:pointer-events-none disabled:opacity-40",
    "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg]:text-current",
    variants[variant],
    sizes[size],
    className,
  );
}
