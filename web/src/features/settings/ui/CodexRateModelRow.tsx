import { Trash2 } from "lucide-react";

import type { CodexRateModelDraft } from "../model/codex-rate-card-draft";
import { modelFieldKey } from "../model/codex-rate-card-draft";
import { CodexRateTierFields } from "./CodexRateTierFields";
import { IconButton } from "@/shared/ui/IconButton";
import { Select } from "@/shared/ui/Select";
import { Switch } from "@/shared/ui/Switch";

interface CodexRateModelRowProps {
  value: CodexRateModelDraft;
  modelOptions: string[];
  selectedModels: Set<string>;
  errors: Record<string, string>;
  disabled: boolean;
  onChange: (value: CodexRateModelDraft) => void;
  onRemove: () => void;
}

export function CodexRateModelRow({
  value,
  modelOptions,
  selectedModels,
  errors,
  disabled,
  onChange,
  onRemove,
}: CodexRateModelRowProps) {
  const nameError = errors[modelFieldKey(value.localId, "name")];
  const modelLabel = value.model.trim() || "新模型";
  const nameId = `${value.localId}-name`;

  return (
    <section className="py-4" aria-label={`${modelLabel} 费率`}>
      <div className="flex items-end gap-2">
        <label htmlFor={nameId} className="min-w-0 w-full max-w-[22rem]">
          <span className="mb-1 block text-[11px] font-medium text-secondary">模型名称</span>
          <Select
            id={nameId}
            className="font-mono"
            value={value.model}
            options={modelOptions.map((model) => ({
              value: model,
              label: model,
              disabled: model !== value.model && selectedModels.has(model),
            }))}
            placeholder="选择模型"
            invalid={Boolean(nameError)}
            disabled={disabled}
            aria-label="模型名称"
            aria-describedby={nameError ? `${nameId}-error` : undefined}
            onValueChange={(model) => onChange({ ...value, model })}
          />
          {nameError ? (
            <span id={`${nameId}-error`} className="mt-1 block text-[10px] text-danger" role="alert">
              {nameError}
            </span>
          ) : null}
        </label>
        <IconButton
          className="mt-5"
          label={`删除 ${modelLabel}`}
          tone="danger"
          disabled={disabled}
          onClick={onRemove}
        >
          <Trash2 size={14} />
        </IconButton>
      </div>

      <div className="mt-2 max-w-[40rem] rounded-[8px] bg-surface-muted/45 px-2.5 py-2">
        <p className="mb-1.5 text-[11px] font-medium text-secondary">标准档</p>
        <CodexRateTierFields
          localId={value.localId}
          modelLabel={modelLabel}
          tier="standard"
          value={value.standard}
          errors={errors}
          disabled={disabled}
          onChange={(standard) => onChange({ ...value, standard })}
        />
      </div>

      <div className="mt-1.5 max-w-[40rem] rounded-[8px] bg-surface-muted/45 px-2.5 py-2">
        <div className="mb-1.5 flex items-center justify-between gap-3">
          <label
            htmlFor={`${value.localId}-fast-enabled`}
            className="text-[11px] font-medium text-secondary"
          >
            快速档
          </label>
          <Switch
            id={`${value.localId}-fast-enabled`}
            checked={value.fastEnabled}
            disabled={disabled}
            aria-label={`${modelLabel} 快速档`}
            onCheckedChange={(fastEnabled) => onChange({ ...value, fastEnabled })}
          />
        </div>
        {value.fastEnabled ? (
          <CodexRateTierFields
            localId={value.localId}
            modelLabel={modelLabel}
            tier="fast"
            value={value.fast}
            errors={errors}
            disabled={disabled}
            onChange={(fast) => onChange({ ...value, fast })}
          />
        ) : null}
      </div>
    </section>
  );
}
