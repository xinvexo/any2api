import { Plus } from "lucide-react";

import type { CodexRateCardValue, SettingItem } from "../api/settings-contracts";
import {
  createEmptyCodexRateModelDraft,
  type CodexRateCardDraft,
} from "../model/codex-rate-card-draft";
import { CodexRateModelRow } from "./CodexRateModelRow";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";

interface CodexRateCardFormProps {
  item: SettingItem;
  value: CodexRateCardDraft;
  errors: Record<string, string>;
  disabled: boolean;
  onChange: (value: CodexRateCardDraft) => void;
}

export function CodexRateCardForm({
  item,
  value,
  errors,
  disabled,
  onChange,
}: CodexRateCardFormProps) {
  const defaultCard = item.defaultValue as CodexRateCardValue;
  const overrideCard = item.overrideValue as CodexRateCardValue | null;
  const effectiveCard = item.effectiveValue as CodexRateCardValue;

  function updateModel(localId: string, model: CodexRateCardDraft["models"][number]) {
    onChange({
      ...value,
      models: value.models.map((current) => current.localId === localId ? model : current),
    });
  }

  function removeModel(localId: string) {
    onChange({ ...value, models: value.models.filter((model) => model.localId !== localId) });
  }

  function addModel() {
    const localId = nextLocalId(value);
    onChange({
      ...value,
      models: [...value.models, createEmptyCodexRateModelDraft(localId)],
    });
  }

  return (
    <div>
      <div className="flex flex-wrap gap-x-5 gap-y-1 border-b border-subtle py-3 text-[11px] text-tertiary">
        <p>默认 <span className="text-secondary">{summarizeCard(defaultCard)}</span></p>
        <p>覆盖 <span className="text-secondary">{overrideCard ? summarizeCard(overrideCard) : "未设置"}</span></p>
        <p>生效 <span className="text-secondary">{summarizeCard(effectiveCard)}</span></p>
      </div>

      <section className="border-b border-subtle py-5" aria-labelledby="codex-rate-exchange-heading">
        <h2 id="codex-rate-exchange-heading" className="text-[14px] font-semibold tracking-tight">
          美元换算
        </h2>
        <label htmlFor="codex-credits-per-usd" className="mt-3 block max-w-xs">
          <span className="mb-1 block text-[11px] text-secondary">Credits / USD</span>
          <input
            id="codex-credits-per-usd"
            className={controlClass(Boolean(errors.creditsPerUsd), "tabular-nums")}
            type="text"
            inputMode="numeric"
            pattern="[0-9]*"
            value={value.creditsPerUsd}
            disabled={disabled}
            aria-invalid={Boolean(errors.creditsPerUsd)}
            aria-describedby={errors.creditsPerUsd ? "codex-credits-per-usd-error" : undefined}
            onChange={(event) => onChange({ ...value, creditsPerUsd: event.target.value })}
          />
          {errors.creditsPerUsd ? (
            <span id="codex-credits-per-usd-error" className="mt-1 block text-[10px] text-danger" role="alert">
              {errors.creditsPerUsd}
            </span>
          ) : null}
        </label>
      </section>

      <section className="py-5" aria-labelledby="codex-model-rates-heading">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 id="codex-model-rates-heading" className="text-[14px] font-semibold tracking-tight">
              模型费率
            </h2>
            <p className="mt-0.5 text-[11px] text-tertiary">Credits / 百万 Token</p>
          </div>
          <Button variant="secondary" size="sm" disabled={disabled} onClick={addModel}>
            <Plus size={14} />
            添加模型
          </Button>
        </div>
        {errors.models ? <p className="mt-3 text-[12px] text-danger" role="alert">{errors.models}</p> : null}
        <div className="mt-2 divide-y divide-subtle">
          {value.models.map((model) => (
            <CodexRateModelRow
              key={model.localId}
              value={model}
              errors={errors}
              disabled={disabled}
              onChange={(next) => updateModel(model.localId, next)}
              onRemove={() => removeModel(model.localId)}
            />
          ))}
        </div>
      </section>
    </div>
  );
}

function summarizeCard(card: CodexRateCardValue) {
  return `${card.credits_per_usd} Credits / $1 · ${Object.keys(card.models).length} 个模型`;
}

function nextLocalId(draft: CodexRateCardDraft) {
  let index = draft.models.length + 1;
  while (draft.models.some((model) => model.localId === `model-${index}`)) index += 1;
  return `model-${index}`;
}
