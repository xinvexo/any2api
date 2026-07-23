import { ClaudeIcon, OpenAiIcon, type BrandIcon } from "@/shared/icons/brand-icons";

import type { ProviderKind } from "../api/provider-contracts";

export interface ProviderKindOption {
  kind: ProviderKind;
  label: string;
  icon: BrandIcon;
}

/** Supported provider kinds shown in the admin UI. Extend when new kinds ship. */
export const PROVIDER_KIND_OPTIONS: readonly ProviderKindOption[] = [
  { kind: "codex", label: "Codex", icon: OpenAiIcon },
  { kind: "claude", label: "Claude", icon: ClaudeIcon },
] as const;

export function isProviderKind(value: string | null | undefined): value is ProviderKind {
  return PROVIDER_KIND_OPTIONS.some((option) => option.kind === value);
}

export function providerKindLabel(kind: ProviderKind): string {
  return PROVIDER_KIND_OPTIONS.find((option) => option.kind === kind)?.label ?? kind;
}
