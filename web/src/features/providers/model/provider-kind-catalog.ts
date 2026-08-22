import {
  ClaudeIcon,
  GrokIcon,
  KimiIcon,
  OpenAiIcon,
  type BrandIcon,
} from "@/shared/icons/brand-icons";
import { providerKindLabel, type ProviderKind } from "@/shared/api/provider-protocol-vocabulary";

export interface ProviderKindOption {
  kind: ProviderKind;
  label: string;
  icon: BrandIcon;
}

/** Supported provider kinds shown in the admin UI. Extend when new kinds ship. */
export const PROVIDER_KIND_OPTIONS: readonly ProviderKindOption[] = [
  { kind: "codex", label: providerKindLabel("codex"), icon: OpenAiIcon },
  { kind: "claude", label: providerKindLabel("claude"), icon: ClaudeIcon },
  { kind: "grok", label: providerKindLabel("grok"), icon: GrokIcon },
  { kind: "kimi", label: providerKindLabel("kimi"), icon: KimiIcon },
  { kind: "openai", label: providerKindLabel("openai"), icon: OpenAiIcon },
] as const;
