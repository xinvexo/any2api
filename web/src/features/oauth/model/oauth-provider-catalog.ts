import { ClaudeIcon, OpenAiIcon, type BrandIcon } from "@/shared/icons/brand-icons";

import type { OAuthProvider } from "../api/oauth-contracts";

export interface OAuthProviderOption {
  provider: OAuthProvider;
  label: string;
  icon: BrandIcon;
}

/** Providers supported by the standalone OAuth2 login tool. */
export const OAUTH_PROVIDER_OPTIONS: readonly OAuthProviderOption[] = [
  { provider: "codex", label: "Codex", icon: OpenAiIcon },
  { provider: "claude", label: "Claude", icon: ClaudeIcon },
] as const;

export function isOAuthProvider(value: string | null | undefined): value is OAuthProvider {
  return OAUTH_PROVIDER_OPTIONS.some((option) => option.provider === value);
}

export function oauthProviderLabel(provider: OAuthProvider): string {
  return OAUTH_PROVIDER_OPTIONS.find((option) => option.provider === provider)?.label ?? provider;
}
