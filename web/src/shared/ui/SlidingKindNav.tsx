import type { BrandIcon } from "@/shared/icons/brand-icons";
import { cn } from "@/shared/lib/cn";
import { SlidingSelectionIndicator } from "@/shared/ui/SlidingSelectionIndicator";

export interface SlidingKindNavOption<Value extends string> {
  value: Value;
  label: string;
  icon: BrandIcon;
}

interface SlidingKindNavProps<Value extends string> {
  ariaLabel: string;
  selected: Value;
  options: readonly SlidingKindNavOption<Value>[];
  counts: Record<Value, number>;
  disabled?: boolean;
  onSelect: (value: Value) => void;
}

export function SlidingKindNav<Value extends string>({
  ariaLabel,
  selected,
  options,
  counts,
  disabled = false,
  onSelect,
}: SlidingKindNavProps<Value>) {
  return (
    <nav aria-label={ariaLabel} className="min-w-0">
      <ul
        className="page-tabs relative isolate flex gap-1 overflow-x-auto rounded-[12px] bg-surface-muted/55 p-1 xl:flex-col xl:gap-1.5 xl:overflow-x-visible xl:bg-transparent xl:p-0"
      >
        <SlidingSelectionIndicator
          selected={selected}
          className="rounded-[10px] bg-nav-active xl:rounded-[12px]"
        />

        {options.map((option) => {
          const active = selected === option.value;
          const Icon = option.icon;
          return (
            <li
              key={option.value}
              data-sliding-selection-item={option.value}
              className="relative z-10 min-w-0 shrink-0 grow xl:grow-0"
            >
              <button
                type="button"
                aria-current={active ? "page" : undefined}
                disabled={disabled}
                onClick={() => onSelect(option.value)}
                className={cn(
                  "group focus-ring flex h-9 w-full items-center gap-2 rounded-[10px] px-2.5 text-left transition-colors duration-200 pointer-coarse:min-h-10 xl:h-11 xl:gap-2.5 xl:rounded-[12px] xl:px-3",
                  "disabled:pointer-events-none disabled:opacity-50",
                  active ? "text-nav-active-fg" : "text-secondary hover:text-primary",
                )}
              >
                <Icon
                  size={16}
                  className={cn(
                    "shrink-0 transition-colors duration-200",
                    active ? "text-primary" : "text-secondary group-hover:text-primary",
                  )}
                />
                <span className="min-w-0 flex-1 whitespace-nowrap text-[13px] font-semibold tracking-tight xl:truncate xl:text-[14px]">
                  {option.label}
                </span>
                <span
                  className={cn(
                    "shrink-0 tabular-nums text-xs font-medium transition-colors duration-200",
                    active ? "text-secondary" : "text-tertiary group-hover:text-secondary",
                  )}
                >
                  {counts[option.value] ?? 0}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
