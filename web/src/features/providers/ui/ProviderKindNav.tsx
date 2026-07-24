import type { ProviderKind } from "../api/provider-contracts";
import {
  PROVIDER_KIND_OPTIONS,
  type ProviderKindOption,
} from "../model/provider-kind-catalog";
import { cn } from "@/shared/lib/cn";

interface ProviderKindNavProps {
  selected: ProviderKind;
  counts: Record<ProviderKind, number>;
  onSelect: (kind: ProviderKind) => void;
}

export function ProviderKindNav({ selected, counts, onSelect }: ProviderKindNavProps) {
  return (
    <nav aria-label="Provider 类型" className="min-w-0">
      {/* Mobile: equal-width segmented control. Desktop: vertical rail. */}
      <ul className="grid grid-cols-2 gap-1 rounded-[12px] bg-surface-muted/55 p-1 sm:flex sm:flex-col sm:gap-1.5 sm:bg-transparent sm:p-0">
        {PROVIDER_KIND_OPTIONS.map((option) => (
          <li key={option.kind} className="min-w-0">
            <KindButton
              option={option}
              count={counts[option.kind] ?? 0}
              active={selected === option.kind}
              onSelect={onSelect}
            />
          </li>
        ))}
      </ul>
    </nav>
  );
}

function KindButton({
  option,
  count,
  active,
  onSelect,
}: {
  option: ProviderKindOption;
  count: number;
  active: boolean;
  onSelect: (kind: ProviderKind) => void;
}) {
  const Icon = option.icon;
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={() => onSelect(option.kind)}
      className={cn(
        "focus-ring flex h-9 w-full items-center gap-2 rounded-[10px] px-2.5 text-left transition-colors sm:h-11 sm:gap-2.5 sm:rounded-[12px] sm:px-3",
        active
          ? "bg-surface text-primary shadow-sm sm:bg-surface-muted sm:shadow-none"
          : "text-secondary hover:bg-surface/70 hover:text-primary sm:hover:bg-surface-muted/70",
      )}
    >
      <Icon size={16} className={cn("shrink-0", active ? "text-primary" : "text-secondary")} />
      <span className="min-w-0 flex-1 truncate text-[13px] font-semibold tracking-tight sm:text-[14px]">
        {option.label}
      </span>
      <span
        className={cn(
          "shrink-0 tabular-nums text-[11px] sm:text-[12px]",
          active ? "text-secondary" : "text-tertiary",
        )}
      >
        {count}
      </span>
    </button>
  );
}
