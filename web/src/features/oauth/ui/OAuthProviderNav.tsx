import type { OAuthProvider } from "../api/oauth-contracts";
import { OAUTH_PROVIDER_OPTIONS } from "../model/oauth-provider-catalog";
import { SlidingKindNav } from "@/shared/ui/SlidingKindNav";

const NAV_OPTIONS = OAUTH_PROVIDER_OPTIONS.map((option) => ({
  value: option.provider,
  label: option.label,
  icon: option.icon,
}));

interface OAuthProviderNavProps {
  selected: OAuthProvider;
  counts: Record<OAuthProvider, number>;
  disabled?: boolean;
  onSelect: (provider: OAuthProvider) => void;
}

export function OAuthProviderNav({
  selected,
  counts,
  disabled = false,
  onSelect,
}: OAuthProviderNavProps) {
  return (
    <SlidingKindNav
      ariaLabel="OAuth2 类型"
      selected={selected}
      options={NAV_OPTIONS}
      counts={counts}
      disabled={disabled}
      onSelect={onSelect}
    />
  );
}
