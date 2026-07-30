import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";

import type {
  SettingItem,
  SettingsConfiguration,
  SettingValue,
} from "../api/settings-contracts";
import { applySettingChanges } from "../api/settings-api";
import {
  createSettingDraft,
  isSettingDraftDirty,
  type SettingDraft,
  validateSettingDraft,
} from "./setting-draft";
import { selectNewestSettingsConfiguration } from "./settings-cache";
import { settingsQueryKeys } from "./settings-query-keys";
import { useSettings } from "./use-settings";

type PendingDrafts = Record<string, SettingDraft>;

export function useSettingsEditor(webGroups?: readonly string[]) {
  const query = useSettings();
  const queryClient = useQueryClient();
  const [drafts, setDrafts] = useState<PendingDrafts>({});
  const items = useMemo(() => {
    const allowed = webGroups ? new Set(webGroups) : null;
    return (query.data?.items ?? []).filter((item) => !allowed || allowed.has(item.webGroup));
  }, [query.data, webGroups]);
  const mutation = useMutation({
    mutationFn: applySettingChanges,
    onSuccess: (configuration) => {
      queryClient.setQueryData<SettingsConfiguration>(settingsQueryKeys.list(), (current) =>
        selectNewestSettingsConfiguration(current, configuration));
      void queryClient.invalidateQueries({ queryKey: settingsQueryKeys.all });
    },
    onError: () => queryClient.refetchQueries({
      queryKey: settingsQueryKeys.all,
      type: "active",
    }),
    retry: false,
  });

  const dirtyItems = items.filter((item) => itemIsDirty(item, drafts));
  const hasValidationErrors = dirtyItems.some((item) => {
    const draft = drafts[item.key];
    return draft !== undefined && validateSettingDraft(item, draft).error !== null;
  });

  function setDraft(item: SettingItem, draft: SettingDraft) {
    mutation.reset();
    setDrafts((current) => withDraft(current, item, draft));
  }

  function discard() {
    mutation.reset();
    setDrafts({});
  }

  async function refresh() {
    discard();
    await query.refetch();
  }

  async function save() {
    const configuration = query.data;
    if (!configuration || dirtyItems.length === 0 || hasValidationErrors) {
      return false;
    }
    const updates: Array<{ key: string; value: SettingValue }> = [];
    for (const item of dirtyItems) {
      const draft = drafts[item.key];
      if (draft === undefined) return false;
      const validation = validateSettingDraft(item, draft);
      if (validation.value === undefined) {
        return false;
      }
      updates.push({ key: item.key, value: validation.value });
    }
    mutation.reset();
    try {
      await mutation.mutateAsync({
        expectedRevision: configuration.configRevision,
        updates,
      });
      setDrafts({});
      return true;
    } catch {
      return false;
    }
  }

  return {
    query,
    items,
    pending: query.isFetching || mutation.isPending,
    isSaving: mutation.isPending,
    isDirty: dirtyItems.length > 0,
    hasValidationErrors,
    saveError: mutation.error,
    draftFor: (item: SettingItem) => draftFor(item, drafts),
    isItemDirty: (item: SettingItem) => itemIsDirty(item, drafts),
    setDraft,
    discard,
    refresh,
    save,
  };
}

export type SettingsEditor = ReturnType<typeof useSettingsEditor>;

function draftFor(item: SettingItem, drafts: PendingDrafts) {
  if (!hasDraft(drafts, item.key)) {
    return createSettingDraft(item);
  }
  return drafts[item.key] ?? createSettingDraft(item);
}

function itemIsDirty(item: SettingItem, drafts: PendingDrafts) {
  if (!hasDraft(drafts, item.key)) {
    return false;
  }
  const draft = drafts[item.key];
  return draft !== undefined && isSettingDraftDirty(item, draft);
}

function withDraft(current: PendingDrafts, item: SettingItem, draft: SettingDraft) {
  const next = { ...current };
  if (isSettingDraftDirty(item, draft)) {
    next[item.key] = draft;
  } else {
    delete next[item.key];
  }
  return next;
}

function hasDraft(drafts: PendingDrafts, key: string) {
  return Object.prototype.hasOwnProperty.call(drafts, key);
}
