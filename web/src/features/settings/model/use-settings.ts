import { useQuery } from "@tanstack/react-query";

import { listSettings } from "../api/settings-api";
import { settingsQueryKeys } from "./settings-query-keys";

export function useSettings() {
  return useQuery({
    queryKey: settingsQueryKeys.list(),
    queryFn: ({ signal }) => listSettings(signal),
  });
}
