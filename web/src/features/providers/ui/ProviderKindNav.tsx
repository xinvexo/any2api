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
      <p className="mb-2 hidden px-1 text-[11px] font-medium uppercase tracking-wide text-tertiary sm:block">
        类型
      </p>
      <ul className="flex gap-1.5 overflow-x-auto pb-0.5 sm:flex-col sm:gap-0.5 sm:overflow-visible">
        {PROVIDER_KIND_OPTIONS.map((option) => (
          <li key={option.kind} className="shrink-0 sm:shrink">
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
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={() => onSelect(option.kind)}
      className={cn(
        "focus-ring flex w-full items-center gap-2 rounded-[10px] px-2.5 py-2 text-left transition-colors",
        active
          ? "bg-surface-muted text-primary"
          : "text-secondary hover:bg-surface-muted/70 hover:text-primary",
      )}
    >
      <span className="min-w-0 flex-1">
        <span className="block text-[13px] font-semibold tracking-tight">{option.label}</span>
        <span className="mt-0.5 hidden text-[11px] text-tertiary sm:block">
          {option.description}
        </span>
      </span>
      <span
        className={cn(
          "tabular-nums text-[11px]",
          active ? "text-secondary" : "text-tertiary",
        )}
      >
        {count}
      </span>
    </button>
  );
}
