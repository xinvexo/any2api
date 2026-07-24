import type { OAuthProvider } from "../api/oauth-contracts";
import {
  OAUTH_PROVIDER_OPTIONS,
  type OAuthProviderOption,
} from "../model/oauth-provider-catalog";
import { cn } from "@/shared/lib/cn";

interface OAuthProviderNavProps {
  selected: OAuthProvider;
  counts: Record<OAuthProvider, number>;
  onSelect: (provider: OAuthProvider) => void;
}

export function OAuthProviderNav({ selected, counts, onSelect }: OAuthProviderNavProps) {
  return (
    <nav aria-label="OAuth2 类型" className="min-w-0">
      {/* Mobile: equal-width segmented control. Desktop: vertical rail. */}
      <ul className="grid grid-cols-2 gap-1 rounded-[12px] bg-surface-muted/55 p-1 sm:flex sm:flex-col sm:gap-1.5 sm:bg-transparent sm:p-0">
        {OAUTH_PROVIDER_OPTIONS.map((option) => (
          <li key={option.provider} className="min-w-0">
            <ProviderButton
              option={option}
              count={counts[option.provider] ?? 0}
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
  count,
  active,
  onSelect,
}: {
  option: OAuthProviderOption;
  count: number;
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
