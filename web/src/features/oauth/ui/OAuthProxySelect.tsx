import type { OAuthProxySelection } from "../api/oauth-contracts";
import { describeProxyProfile } from "../model/oauth-proxy-presentation";
import type { ProxyConfiguration } from "@/features/proxies";
import { Select, type SelectOption } from "@/shared/ui/Select";

const GLOBAL_VALUE = "oauth-global";

export function OAuthProxySelect({
  id,
  selection,
  configuration,
  disabled = false,
  onChange,
}: {
  id: string;
  selection: OAuthProxySelection;
  configuration: ProxyConfiguration;
  disabled?: boolean;
  onChange: (selection: OAuthProxySelection) => void;
}) {
  const globalProxy = configuration.items.find(
    (profile) => profile.id === configuration.globalProxyId,
  );
  const options: SelectOption<string>[] = [
    {
      value: GLOBAL_VALUE,
      label: `跟随全局（${describeProxyProfile(globalProxy)}）`,
    },
    ...configuration.items.map((profile) => ({
      value: profile.id,
      label: `${describeProxyProfile(profile)}${profile.enabled ? "" : "（已停用）"}`,
      disabled: !profile.enabled,
    })),
  ];
  const value = selection.mode === "global" ? GLOBAL_VALUE : selection.proxyProfileId;

  return (
    <Select
      id={id}
      aria-label="OAuth 出口"
      value={value}
      options={options}
      disabled={disabled}
      onValueChange={(next) =>
        onChange(
          next === GLOBAL_VALUE
            ? { mode: "global" }
            : { mode: "profile", proxyProfileId: next },
        )
      }
    />
  );
}
