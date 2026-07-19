import type { SettingsConfiguration } from "../api/settings-contracts";

export function selectNewestSettingsConfiguration(
  current: SettingsConfiguration | undefined,
  incoming: SettingsConfiguration,
) {
  if (!current || incoming.configRevision >= current.configRevision) {
    return incoming;
  }
  return current;
}
