import type { ModelRouteConfiguration } from "../api/model-route-contracts";

export function selectNewestModelRouteConfiguration(
  current: ModelRouteConfiguration | undefined,
  incoming: ModelRouteConfiguration,
) {
  if (!current || incoming.configRevision >= current.configRevision) {
    return incoming;
  }
  return current;
}
