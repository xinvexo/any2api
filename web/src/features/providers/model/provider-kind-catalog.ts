import type { ProviderKind } from "../api/provider-contracts";

export interface ProviderKindOption {
  kind: ProviderKind;
  label: string;
  description: string;
}

/** Supported provider kinds shown in the admin UI. Extend when new kinds ship. */
export const PROVIDER_KIND_OPTIONS: readonly ProviderKindOption[] = [
  { kind: "codex", label: "Codex", description: "OpenAI Responses" },
  { kind: "claude", label: "Claude", description: "Anthropic Messages" },
] as const;

export function isProviderKind(value: string | null | undefined): value is ProviderKind {
  return PROVIDER_KIND_OPTIONS.some((option) => option.kind === value);
}

export function providerKindLabel(kind: ProviderKind): string {
  return PROVIDER_KIND_OPTIONS.find((option) => option.kind === kind)?.label ?? kind;
}
