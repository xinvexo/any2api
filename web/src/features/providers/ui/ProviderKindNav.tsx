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
      <ul className="flex gap-2 overflow-x-auto sm:flex-col sm:gap-1.5 sm:overflow-visible">
        {PROVIDER_KIND_OPTIONS.map((option) => (
          <li key={option.kind} className="shrink-0 sm:shrink sm:w-full">
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
        "focus-ring flex h-11 min-w-[8.5rem] w-full items-center gap-2.5 rounded-[12px] px-3 text-left transition-colors sm:min-w-0",
        active
          ? "bg-surface-muted text-primary"
          : "text-secondary hover:bg-surface-muted/70 hover:text-primary",
      )}
    >
      <Icon size={18} className={cn(active ? "text-primary" : "text-secondary")} />
      <span className="min-w-0 flex-1 truncate text-[14px] font-semibold tracking-tight">
        {option.label}
      </span>
      <span
        className={cn(
          "tabular-nums text-[12px]",
          active ? "text-secondary" : "text-tertiary",
        )}
      >
        {count}
      </span>
    </button>
  );
}
