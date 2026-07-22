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
import { reloadLabel, settingLabel } from "./setting-presentation";

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
  const restartHint = reloadLabel(item);
  const showActions = dirty || item.overrideValue !== null;

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
      className="grid gap-3 border-b border-subtle px-1 py-3.5 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_minmax(200px,240px)] sm:items-center sm:gap-6"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <div className="min-w-0">
        <h3 id={headingId} className="text-[13px] font-medium text-primary">
          {label}
        </h3>
        <p id={descriptionId} className="mt-0.5 text-[12px] leading-5 text-secondary">
          {item.description}
        </p>
        {restartHint ? <p className="mt-1 text-[11px] text-warning">{restartHint}</p> : null}
        {errorMessage ? (
          <p id={errorId} className="mt-1.5 text-[12px] text-danger" role="alert">
            {errorMessage}
          </p>
        ) : null}
      </div>

      <div className="flex min-w-0 flex-col items-stretch gap-2">
        <SettingControl
          item={item}
          value={value}
          disabled={pending}
          invalid={validation.error !== null}
          labelledBy={headingId}
          describedBy={describedBy}
          onChange={setDraft}
        />

        {showActions ? (
          <div className="flex flex-wrap items-center justify-end gap-1">
            {item.overrideValue !== null ? (
              <Button
                type="button"
                variant="ghost"
                className="h-7 px-2"
                onClick={() => void reset()}
                disabled={pending}
                aria-label={`恢复${label}默认值`}
              >
                <RotateCcw size={13} />
                恢复默认
              </Button>
            ) : null}
            {dirty ? (
              <Button
                type="submit"
                variant="primary"
                className="h-7 px-2"
                disabled={pending || validation.error !== null}
                aria-label={`保存${label}`}
              >
                <Check size={13} />
                保存
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    </form>
  );
}
