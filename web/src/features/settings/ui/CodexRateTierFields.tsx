import type { CodexRateTierDraft } from "../model/codex-rate-card-draft";
import { tierFieldKey } from "../model/codex-rate-card-draft";
import { controlClass } from "@/shared/ui/form-control";

interface CodexRateTierFieldsProps {
  localId: string;
  modelLabel: string;
  tier: "standard" | "fast";
  value: CodexRateTierDraft;
  errors: Record<string, string>;
  disabled: boolean;
  onChange: (value: CodexRateTierDraft) => void;
}

const FIELDS = [
  { key: "input", label: "输入" },
  { key: "cachedInput", label: "缓存输入" },
  { key: "output", label: "输出" },
] as const;

export function CodexRateTierFields({
  localId,
  modelLabel,
  tier,
  value,
  errors,
  disabled,
  onChange,
}: CodexRateTierFieldsProps) {
  const tierLabel = tier === "standard" ? "标准" : "快速";
  return (
    <div className="grid grid-cols-3 gap-1.5">
      {FIELDS.map(({ key, label }) => {
        const error = errors[tierFieldKey(localId, tier, key)];
        const inputId = `${localId}-${tier}-${key}`;
        return (
          <label key={key} htmlFor={inputId} className="min-w-0">
            <span className="mb-1 block text-xs text-secondary">{label}</span>
            <input
              id={inputId}
              className={controlClass(Boolean(error), "h-7 tabular-nums")}
              type="text"
              inputMode="decimal"
              value={value[key]}
              disabled={disabled}
              aria-label={`${modelLabel} ${tierLabel}${label}费率`}
              aria-invalid={Boolean(error)}
              aria-describedby={error ? `${inputId}-error` : undefined}
              onChange={(event) => onChange({ ...value, [key]: event.target.value })}
            />
            {error ? (
              <span id={`${inputId}-error`} className="mt-1 block text-xs text-danger" role="alert">
                {error}
              </span>
            ) : null}
          </label>
        );
      })}
    </div>
  );
}
