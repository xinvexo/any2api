import { useMutation } from "@tanstack/react-query";
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
import { settingsQueryKeys } from "./settings-query-keys";
import { useSettings } from "./use-settings";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";
import {
  draftSourceChanged,
  type VersionedDraft,
} from "@/shared/lib/versioned-draft";

type PendingDrafts = Record<string, SettingDraft>;
const EMPTY_DRAFTS: PendingDrafts = {};

export function useSettingsEditor(webGroups?: readonly string[]) {
  const query = useSettings();
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<SettingsConfiguration>({
      cacheKey: settingsQueryKeys.list(),
      invalidateKey: settingsQueryKeys.all,
      refreshKey: settingsQueryKeys.all,
    });
  const [pendingDraft, setPendingDraft] = useState<VersionedDraft<PendingDrafts> | null>(null);
  const drafts = pendingDraft?.value ?? EMPTY_DRAFTS;
  const items = useMemo(() => {
    const allowed = webGroups ? new Set(webGroups) : null;
    return (query.data?.items ?? []).filter((item) =>
      item.valueType !== "codex_rate_card"
      && (!allowed || allowed.has(item.webGroup))
    );
  }, [query.data, webGroups]);
  const mutation = useMutation({
    mutationFn: applySettingChanges,
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });
  const hasSourceConflict = Boolean(
    pendingDraft
    && query.data
    && draftSourceChanged(pendingDraft.source, {
      configRevision: query.data.configRevision,
    }),
  );

  const dirtyItems = items.filter((item) => itemIsDirty(item, drafts));
  const hasValidationErrors = dirtyItems.some((item) => {
    const draft = drafts[item.key];
    return draft !== undefined && validateSettingDraft(item, draft).error !== null;
  });

  function setDraft(item: SettingItem, draft: SettingDraft) {
    const configRevision = query.data?.configRevision;
    if (configRevision === undefined) {
      return;
    }
    mutation.reset();
    setPendingDraft((current) => {
      const next = withDraft(current?.value ?? EMPTY_DRAFTS, item, draft);
      if (Object.keys(next).length === 0) {
        return null;
      }
      return {
        source: current?.source ?? { configRevision },
        value: next,
      };
    });
  }

  function discard() {
    mutation.reset();
    setPendingDraft(null);
  }

  async function refresh() {
    discard();
    const result = await query.refetch();
    return result.isSuccess;
  }

  async function save() {
    const configuration = query.data;
    if (
      !configuration
      || !pendingDraft
      || dirtyItems.length === 0
      || hasValidationErrors
      || hasSourceConflict
    ) {
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
        expectedRevision: pendingDraft.source.configRevision,
        updates,
      });
      setPendingDraft(null);
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
    isDirty: pendingDraft !== null,
    hasValidationErrors,
    hasSourceConflict,
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
