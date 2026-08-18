import { useState } from "react";

import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import {
  formatSettingUnitValue,
  parseSettingUnitValue,
  preferredSettingUnit,
  sanitizeSettingUnitInput,
  settingUnitKind,
  settingUnitOptions,
} from "./setting-units";
import { Select } from "@/shared/ui/Select";

interface ScaledNumericControlProps {
  item: SettingItem;
  value: SettingDraft;
  disabled: boolean;
  invalid: boolean;
  labelledBy: string;
  describedBy?: string;
  onChange: (value: SettingDraft) => void;
}

export function ScaledNumericControl({
  item,
  value,
  disabled,
  invalid,
  labelledBy,
  describedBy,
  onChange,
}: ScaledNumericControlProps) {
  const kind = settingUnitKind(item) ?? "duration";
  const options = settingUnitOptions(kind);
  const [unitValue, setUnitValue] = useState(() => {
    const rawValue = typeof value === "string" && /^\d+$/u.test(value) ? Number(value) : 0;
    return preferredSettingUnit(item, rawValue).value;
  });
  const unit = options.find((option) => option.value === unitValue) ?? options[0];
  const rawValue = typeof value === "string" ? value : String(value);

  function changeUnit(nextValue: string) {
    setUnitValue(nextValue);
  }

  return (
    <div className="flex min-w-0 items-center gap-2">
      <input
        className="focus-ring h-8 min-w-0 flex-1 rounded-[8px] border-0 bg-surface-muted px-2.5 text-[12px] tabular-nums text-primary placeholder:text-tertiary disabled:cursor-not-allowed disabled:opacity-50"
        type="text"
        inputMode="decimal"
        pattern="[0-9]*(\\.[0-9]+)?"
        value={formatSettingUnitValue(rawValue, unit)}
        placeholder={formatSettingUnitValue(String(item.defaultValue), unit)}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-invalid={invalid}
        disabled={disabled}
        onChange={(event) => onChange(parseSettingUnitValue(sanitizeSettingUnitInput(event.target.value), unit))}
      />
      <Select
        className="w-[6.5rem] shrink-0"
        value={unit.value}
        options={options.map((option) => ({ value: option.value, label: option.label }))}
        aria-label={`${item.key} 单位`}
        aria-describedby={describedBy}
        disabled={disabled}
        onValueChange={changeUnit}
      />
    </div>
  );
}
