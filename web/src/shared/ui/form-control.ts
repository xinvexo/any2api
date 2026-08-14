import { cn } from "@/shared/lib/cn";

const controlClassName =
  "focus-ring h-8 w-full rounded-[8px] border border-subtle bg-surface px-2.5 text-[12px] text-primary transition-colors placeholder:text-tertiary hover:border-strong disabled:bg-surface-muted disabled:opacity-60";

export function controlClass(invalid = false, className?: string) {
  return cn(
    controlClassName,
    invalid && "border-danger/50 bg-danger/[0.05]",
    className,
  );
}
