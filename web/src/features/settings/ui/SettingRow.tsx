import { Check, RotateCcw } from "lucide-react";
import { useId, useState } from "react";

import type { SettingItem, SettingValue } from "../api/settings-contracts";
import {
  createSettingDraft,
  isSettingDraftDirty,
  type SettingDraft,
  validateSettingDraft,
} from "../model/setting-draft";
import { getSettingsErrorMessage } from "../model/settings-error";
import { Button } from "@/shared/ui/Button";
import { SettingControl } from "./SettingControl";
import {
  formatSettingValue,
  reloadLabel,
  settingLabel,
} from "./setting-presentation";

interface SettingRowProps {
  item: SettingItem;
  pending: boolean;
  mutationError: unknown;
  onSave: (item: SettingItem, value: SettingValue) => Promise<void>;
  onReset: (item: SettingItem) => Promise<void>;
}

export function SettingRow({ item, pending, mutationError, onSave, onReset }: SettingRowProps) {
  const [draft, setDraft] = useState<SettingDraft | null>(null);
  const label = settingLabel(item);
  const headingId = useId();
  const descriptionId = useId();
  const errorId = useId();
  const value = draft ?? createSettingDraft(item);
  const validation = validateSettingDraft(item, value);
  const dirty = draft !== null && isSettingDraftDirty(item, draft);
  const errorMessage = validation.error ?? (mutationError ? getSettingsErrorMessage(mutationError) : null);
  const describedBy = errorMessage ? `${descriptionId} ${errorId}` : descriptionId;

  function updateDraft(next: SettingDraft) {
    setDraft(next);
  }

  async function submit() {
    if (!dirty || validation.value === null) {
      return;
    }
    try {
      await onSave(item, validation.value);
      setDraft(null);
    } catch {
      // The mutation owns the error state; keep the draft available for retry.
    }
  }

  async function reset() {
    try {
      await onReset(item);
      setDraft(null);
    } catch {
      // The mutation owns the error state; keep the draft available for retry.
    }
  }

  return (
    <form
      className="grid gap-4 px-5 py-5 lg:grid-cols-[minmax(0,1fr)_minmax(230px,0.8fr)_auto] lg:items-center"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2">
          <h3 id={headingId} className="font-medium">
            {label}
          </h3>
          <span className="rounded-full bg-surface-muted px-2 py-0.5 text-xs text-secondary">
            {item.overrideValue === null ? "默认" : "已覆盖"}
          </span>
        </div>
        <p id={descriptionId} className="mt-1 text-sm leading-5 text-secondary">
          {item.description}
        </p>
        <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-xs text-tertiary sm:grid-cols-[auto_1fr_auto_1fr_auto_1fr]">
          <dt>默认</dt>
          <dd>{formatSettingValue(item.defaultValue, item.valueType)}</dd>
          <dt>覆盖</dt>
          <dd>{formatSettingValue(item.overrideValue, item.valueType)}</dd>
          <dt>生效</dt>
          <dd>{formatSettingValue(item.effectiveValue, item.valueType)}</dd>
        </dl>
        <p className="mt-2 text-xs text-tertiary">{reloadLabel(item)}</p>
      </div>

      <SettingControl
        item={item}
        value={value}
        disabled={pending}
        invalid={validation.error !== null}
        labelledBy={headingId}
        describedBy={describedBy}
        onChange={updateDraft}
      />

      <div className="flex flex-wrap gap-2 lg:justify-end">
        <Button
          type="submit"
          variant={dirty ? "primary" : "secondary"}
          disabled={pending || !dirty || validation.error !== null}
          aria-label={`保存${label}`}
        >
          <Check size={15} />
          保存
        </Button>
        {item.overrideValue !== null ? (
          <Button
            type="button"
            variant="ghost"
            onClick={() => void reset()}
            disabled={pending}
            aria-label={`恢复${label}默认值`}
          >
            <RotateCcw size={15} />
            恢复默认
          </Button>
        ) : null}
      </div>

      {errorMessage ? (
        <p id={errorId} className="text-sm text-danger lg:col-start-1 lg:col-end-4" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </form>
  );
}
