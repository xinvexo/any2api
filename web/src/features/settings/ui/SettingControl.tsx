import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { enumOptionLabel } from "./setting-presentation";

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
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        disabled={disabled}
        className={`focus-ring flex h-10 items-center gap-3 rounded-control border px-3 text-sm disabled:cursor-not-allowed disabled:opacity-50 ${
          checked
            ? "border-accent bg-accent/10 text-accent-copy"
            : "border-subtle bg-surface-muted text-secondary"
        }`}
        onClick={() => onChange(!checked)}
      >
        <span
          aria-hidden="true"
          className={`grid size-5 place-items-center rounded-full ${
            checked ? "bg-accent text-on-accent" : "bg-surface"
          }`}
        >
          {checked ? "✓" : ""}
        </span>
        {checked ? "启用" : "关闭"}
      </button>
    );
  }

  if (item.valueType === "enum") {
    return (
      <select
        className="focus-ring h-10 w-full rounded-control border border-subtle bg-surface px-3 text-sm disabled:cursor-not-allowed disabled:opacity-50"
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
        className="focus-ring h-10 min-w-0 flex-1 rounded-control border border-subtle bg-surface px-3 text-sm tabular-nums disabled:cursor-not-allowed disabled:opacity-50"
        type="text"
        inputMode="numeric"
        pattern="[0-9]*"
        value={String(value)}
        aria-labelledby={labelledBy}
        aria-describedby={describedBy}
        aria-invalid={invalid}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
      />
      <span className="shrink-0 text-xs text-tertiary">
        {item.valueType === "duration_ms"
          ? "毫秒"
          : item.key === "retry.jitter_ratio"
            ? "%"
            : "数量"}
      </span>
    </div>
  );
}
