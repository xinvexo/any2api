import { cn } from "@/shared/lib/cn";

interface SwitchProps {
  id?: string;
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
}

export function Switch({
  id,
  checked,
  disabled = false,
  onCheckedChange,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
  "aria-describedby": ariaDescribedBy,
}: SwitchProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      aria-describedby={ariaDescribedBy}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "focus-ring relative inline-flex h-[20px] w-[36px] shrink-0 items-center rounded-full border transition-colors duration-150",
        "disabled:cursor-not-allowed disabled:opacity-45",
        checked
          ? "border-accent bg-accent"
          : "border-strong bg-surface-hover",
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "pointer-events-none block size-[16px] rounded-full bg-white shadow-sm transition-transform duration-150",
          checked ? "translate-x-[17px]" : "translate-x-[1px]",
        )}
      />
    </button>
  );
}
