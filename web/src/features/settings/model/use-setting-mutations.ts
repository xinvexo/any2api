import { useMutation, useQueryClient } from "@tanstack/react-query";

import type { SettingsConfiguration, SettingWriteInput } from "../api/settings-contracts";
import { resetSetting, updateSetting } from "../api/settings-api";
import { selectNewestSettingsConfiguration } from "./settings-cache";
import { settingsQueryKeys } from "./settings-query-keys";

export function useSettingMutations() {
  const queryClient = useQueryClient();
  const publish = (configuration: SettingsConfiguration) => {
    queryClient.setQueryData<SettingsConfiguration>(settingsQueryKeys.list(), (current) =>
      selectNewestSettingsConfiguration(current, configuration),
    );
    void queryClient.invalidateQueries({ queryKey: settingsQueryKeys.all });
  };
  const refreshAfterFailure = () =>
    queryClient.refetchQueries({ queryKey: settingsQueryKeys.all, type: "active" });
  const update = useMutation({
    mutationFn: ({ key, input }: { key: string; input: SettingWriteInput }) => updateSetting(key, input),
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });
  const reset = useMutation({
    mutationFn: ({ key, expectedRevision }: { key: string; expectedRevision: number }) =>
      resetSetting(key, expectedRevision),
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });
  return {
    update,
    reset,
    isPending: update.isPending || reset.isPending,
  };
}
