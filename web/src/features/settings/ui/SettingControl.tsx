import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { Select } from "@/shared/ui/Select";
import { Switch } from "@/shared/ui/Switch";
import { ModelAllowlistControl } from "./ModelAllowlistControl";
import { enumOptionLabel, formatSettingDefaultPlaceholder } from "./setting-presentation";

interface SettingControlProps {
  item: SettingItem;
  value: SettingDraft;
  disabled: boolean;
  invalid: boolean;
  labelledBy: string;
  describedBy?: string;
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
  if (item.valueType === "model_access") {
    return (
      <ModelAllowlistControl
        item={item}
        value={value}
        disabled={disabled}
        labelledBy={labelledBy}
        describedBy={describedBy}
        onChange={onChange}
      />
    );
  }
  if (item.valueType === "string_list") {
    return (
      <textarea
        className="focus-ring min-h-24 w-full resize-y rounded-[8px] border-0 bg-surface-muted px-3 py-2 font-mono text-[12px] leading-5 text-primary placeholder:text-tertiary disabled:cursor-not-allowed disabled:opacity-50"
        value={typeof value === "string" ? value : ""}
        placeholder={"例如：\n127.0.0.1/32\n10.0.0.0/8"}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-invalid={invalid}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

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
      <Select
        className="min-w-0"
        value={String(value)}
        options={(item.allowedValues ?? []).map((option) => ({
          value: option,
          label: enumOptionLabel(option),
        }))}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        invalid={invalid}
        disabled={disabled}
        onValueChange={onChange}
      />
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
  if (item.key === "logs.file.max_total_size" || item.key === "logs.http_access.max_exchange_bytes") {
    return "字节";
  }
  if (item.key === "retry.jitter_ratio") {
    return "%";
  }
  return "数量";
}
