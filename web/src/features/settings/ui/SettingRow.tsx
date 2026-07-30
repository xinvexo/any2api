import { RotateCcw } from "lucide-react";
import { useId } from "react";

import type { SettingItem } from "../api/settings-contracts";
import { type SettingDraft, validateSettingDraft } from "../model/setting-draft";
import { Button } from "@/shared/ui/Button";
import { SettingControl } from "./SettingControl";
import { reloadLabel, settingLabel } from "./setting-presentation";

interface SettingRowProps {
  item: SettingItem;
  value: SettingDraft;
  pending: boolean;
  resetPending: boolean;
  onChange: (item: SettingItem, value: SettingDraft) => void;
  onReset: (item: SettingItem) => void;
}

export function SettingRow({
  item,
  value,
  pending,
  resetPending,
  onChange,
  onReset,
}: SettingRowProps) {
  const label = settingLabel(item);
  const headingId = useId();
  const descriptionId = useId();
  const errorId = useId();
  const validation = validateSettingDraft(item, value);
  const errorMessage = validation.error;
  const describedBy = errorMessage ? `${descriptionId} ${errorId}` : descriptionId;
  const restartHint = reloadLabel(item);
  const wideControl = item.valueType === "string_list";

  return (
    <div
      className={wideControl
        ? "grid gap-3 px-1 py-3"
        : "grid gap-3 px-1 py-3 sm:grid-cols-[minmax(0,1fr)_minmax(200px,240px)] sm:items-center sm:gap-6"}
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
          onChange={(next) => onChange(item, next)}
        />

        {item.overrideValue !== null && !resetPending ? (
          <div className="flex flex-wrap items-center justify-end gap-1">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => onReset(item)}
              disabled={pending}
              aria-label={`恢复${label}默认值`}
            >
              <RotateCcw size={13} />
              恢复默认
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
