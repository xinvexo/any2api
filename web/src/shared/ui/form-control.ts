import { cn } from "@/shared/lib/cn";

export const controlClassName =
  "focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted px-2.5 text-[12px] text-primary placeholder:text-tertiary disabled:opacity-60";

export function controlClass(invalid = false, className?: string) {
  return cn(
    controlClassName,
    invalid && "bg-danger/[0.05] ring-1 ring-inset ring-danger/40",
    className,
  );
}
