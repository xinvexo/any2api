import { cn } from "@/shared/lib/cn";

export const controlClassName =
  "focus-ring h-8 w-full rounded-[8px] border-0 bg-surface-muted px-2.5 text-[12px] text-primary placeholder:text-tertiary disabled:opacity-60";

/** Select without the native disclosure hover chip (WebKit). */
export const selectClassName =
  "focus-ring field-select h-8 w-full cursor-pointer appearance-none rounded-[8px] border-0 bg-surface-muted py-0 pl-2.5 pr-8 text-[12px] text-primary disabled:opacity-60";

export function controlClass(invalid = false, className?: string) {
  return cn(
    controlClassName,
    invalid && "bg-danger/[0.05] ring-1 ring-inset ring-danger/40",
    className,
  );
}

export function selectClass(invalid = false, className?: string) {
  return cn(
    selectClassName,
    invalid && "bg-danger/[0.05] ring-1 ring-inset ring-danger/40",
    className,
  );
}
