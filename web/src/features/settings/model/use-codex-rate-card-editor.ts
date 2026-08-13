import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import type {
  CodexRateCardValue,
  SettingItem,
  SettingsConfiguration,
} from "../api/settings-contracts";
import { applySettingChanges } from "../api/settings-api";
import {
  createCodexRateCardDraft,
  createVersionedRateCard,
  rateCardContentEqual,
  type CodexRateCardDraft,
  validateCodexRateCardDraft,
} from "./codex-rate-card-draft";
import { settingsQueryKeys } from "./settings-query-keys";
import { useSettings } from "./use-settings";
import { useConfigurationMutationLifecycle } from "@/shared/api/use-configuration-mutation-lifecycle";

const RATE_CARD_KEY = "oauth.codex.rate_card";

interface PendingDraft {
  sourceId: string;
  value: CodexRateCardDraft;
}

export function useCodexRateCardEditor() {
  const query = useSettings();
  const { publish, refreshAfterFailure } =
    useConfigurationMutationLifecycle<SettingsConfiguration>({
      cacheKey: settingsQueryKeys.list(),
      invalidateKey: settingsQueryKeys.all,
      refreshKey: settingsQueryKeys.all,
    });
  const [pendingDraft, setPendingDraft] = useState<PendingDraft | null>(null);
  const mutation = useMutation({
    mutationFn: applySettingChanges,
    onSuccess: publish,
    onError: refreshAfterFailure,
    retry: false,
  });
  const item = findRateCardSetting(query.data?.items ?? []);
  const card = item?.effectiveValue as CodexRateCardValue | undefined;
  const configuration = query.data;
  const activeDraft = card && pendingDraft?.sourceId === card.id ? pendingDraft : null;
  const draft = card ? activeDraft?.value ?? createCodexRateCardDraft(card) : null;
  const validation = draft ? validateCodexRateCardDraft(draft) : null;
  const isDirty = Boolean(
    card
    && activeDraft
    && (!validation?.value || !rateCardContentEqual(validation.value, card)),
  );

  function setDraft(value: CodexRateCardDraft) {
    if (!card || !configuration) return;
    mutation.reset();
    const nextValidation = validateCodexRateCardDraft(value);
    if (nextValidation.value && rateCardContentEqual(nextValidation.value, card)) {
      setPendingDraft(null);
      return;
    }
    setPendingDraft({
      sourceId: card.id,
      value,
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
    if (!card || !configuration || !activeDraft || !validation?.value || !isDirty) return false;
    mutation.reset();
    try {
      await mutation.mutateAsync({
        expectedRevision: configuration.configRevision,
        updates: [{
          key: RATE_CARD_KEY,
          value: createVersionedRateCard(validation.value, card.id),
        }],
      });
      setPendingDraft(null);
      return true;
    } catch {
      return false;
    }
  }

  return {
    query,
    item,
    card,
    draft,
    validation,
    pending: query.isFetching || mutation.isPending,
    isSaving: mutation.isPending,
    isDirty,
    hasValidationErrors: isDirty && validation?.value === null,
    saveError: mutation.error,
    setDraft,
    discard,
    refresh,
    save,
  };
}

export type CodexRateCardEditor = ReturnType<typeof useCodexRateCardEditor>;

function findRateCardSetting(items: SettingItem[]) {
  return items.find((candidate) =>
    candidate.key === RATE_CARD_KEY && candidate.valueType === "codex_rate_card"
  );
}
