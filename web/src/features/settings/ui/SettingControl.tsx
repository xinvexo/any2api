import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { selectClass } from "@/shared/ui/form-control";
import { Switch } from "@/shared/ui/Switch";
import { enumOptionLabel, formatSettingDefaultPlaceholder } from "./setting-presentation";

interface SettingControlProps {
  item: SettingItem;
  value: SettingDraft;
  disabled: boolean;
  invalid: boolean;
  labelledBy: string;
  describedBy: string;
  onChange: (value: SettingDraft) => void;
}

export function SettingControl({
  item,
  value,
  disabled,
  invalid,
  labelledBy,
  describedBy,
  onChange,
}: SettingControlProps) {
  if (item.valueType === "boolean") {
    const checked = value === true;
    return (
      <div className="flex items-center justify-end">
        <Switch
          checked={checked}
          disabled={disabled}
          aria-labelledby={labelledBy}
          aria-describedby={describedBy}
          onCheckedChange={onChange}
        />
      </div>
    );
  }

  if (item.valueType === "enum") {
    return (
      <select
        className={selectClass(invalid, "min-w-0 disabled:cursor-not-allowed disabled:opacity-50")}
        value={String(value)}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-invalid={invalid}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      >
        {item.allowedValues?.map((option) => (
          <option key={option} value={option}>
            {enumOptionLabel(option)}
          </option>
        ))}
      </select>
    );
  }

  return (
    <div className="flex min-w-0 items-center gap-2">
      <input
        className="focus-ring h-8 min-w-0 flex-1 rounded-[8px] border-0 bg-surface-muted px-2.5 text-[12px] tabular-nums text-primary placeholder:text-tertiary disabled:cursor-not-allowed disabled:opacity-50"
        type="text"
        inputMode="numeric"
        pattern="[0-9]*"
        value={String(value)}
        placeholder={formatSettingDefaultPlaceholder(item)}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-invalid={invalid}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
      <span className="shrink-0 text-[11px] text-tertiary">{unitLabel(item)}</span>
    </div>
  );
}

function unitLabel(item: SettingItem) {
  if (item.valueType === "duration_secs") {
    return "秒";
  }
  if (item.key === "logs.file.max_total_size") {
    return "字节";
  }
  if (item.key === "retry.jitter_ratio") {
    return "%";
  }
  return "数量";
}
