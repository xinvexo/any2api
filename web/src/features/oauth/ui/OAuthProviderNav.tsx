import type { OAuthProvider } from "../api/oauth-contracts";
import {
  OAUTH_PROVIDER_OPTIONS,
  type OAuthProviderOption,
} from "../model/oauth-provider-catalog";
import { cn } from "@/shared/lib/cn";

interface OAuthProviderNavProps {
  selected: OAuthProvider;
  onSelect: (provider: OAuthProvider) => void;
}

export function OAuthProviderNav({ selected, onSelect }: OAuthProviderNavProps) {
  return (
    <nav aria-label="OAuth2 类型" className="min-w-0">
      <ul className="flex gap-2 overflow-x-auto sm:flex-col sm:gap-1.5 sm:overflow-visible">
        {OAUTH_PROVIDER_OPTIONS.map((option) => (
          <li key={option.provider} className="shrink-0 sm:shrink sm:w-full">
            <ProviderButton
              option={option}
              active={selected === option.provider}
              onSelect={onSelect}
            />
          </li>
        ))}
      </ul>
    </nav>
  );
}

function ProviderButton({
  option,
  active,
  onSelect,
}: {
  option: OAuthProviderOption;
  active: boolean;
  onSelect: (provider: OAuthProvider) => void;
}) {
  const Icon = option.icon;
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={() => onSelect(option.provider)}
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
    </button>
  );
}
