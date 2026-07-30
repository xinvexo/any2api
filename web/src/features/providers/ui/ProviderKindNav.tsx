import type { ProviderKind } from "../api/provider-contracts";
import { PROVIDER_KIND_OPTIONS } from "../model/provider-kind-catalog";
import { SlidingKindNav } from "@/shared/ui/SlidingKindNav";

const NAV_OPTIONS = PROVIDER_KIND_OPTIONS.map((option) => ({
  value: option.kind,
  label: option.label,
  icon: option.icon,
}));

interface ProviderKindNavProps {
  selected: ProviderKind;
  counts: Record<ProviderKind, number>;
  onSelect: (kind: ProviderKind) => void;
}

export function ProviderKindNav({ selected, counts, onSelect }: ProviderKindNavProps) {
  return (
    <SlidingKindNav
      ariaLabel="Provider 类型"
      selected={selected}
      options={NAV_OPTIONS}
      counts={counts}
      onSelect={onSelect}
    />
  );
}
