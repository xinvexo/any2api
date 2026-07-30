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
  const optionCount = Math.max(options.length, 1);
  return (
    <nav aria-label={ariaLabel} className="min-w-0">
      <ul
        className="relative isolate grid gap-1 rounded-[12px] bg-surface-muted/55 p-1 sm:flex sm:flex-col sm:gap-1.5 sm:bg-transparent sm:p-0"
        style={{ gridTemplateColumns: `repeat(${optionCount}, minmax(0, 1fr))` }}
      >
        <SlidingSelectionIndicator
          selected={selected}
          className="rounded-[10px] bg-nav-active sm:rounded-[12px]"
        />

        {options.map((option) => {
          const active = selected === option.value;
          const Icon = option.icon;
          return (
            <li
              key={option.value}
              data-sliding-selection-item={option.value}
              className="relative z-10 min-w-0"
            >
              <button
                type="button"
                aria-current={active ? "page" : undefined}
                disabled={disabled}
                onClick={() => onSelect(option.value)}
                className={cn(
                  "group focus-ring flex h-9 w-full items-center gap-2 rounded-[10px] px-2.5 text-left transition-colors duration-200 sm:h-11 sm:gap-2.5 sm:rounded-[12px] sm:px-3",
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
                <span className="min-w-0 flex-1 truncate text-[13px] font-semibold tracking-tight sm:text-[14px]">
                  {option.label}
                </span>
                <span
                  className={cn(
                    "shrink-0 tabular-nums text-[11px] font-medium transition-colors duration-200 sm:text-[12px]",
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
