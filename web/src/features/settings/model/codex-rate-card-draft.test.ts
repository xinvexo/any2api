import { expect, test } from "vitest";

import type { CodexRateCardValue } from "../api/settings-contracts";
import {
  createCodexRateCardDraft,
  createVersionedRateCard,
  rateCardContentEqual,
  tierFieldKey,
  validateCodexRateCardDraft,
} from "./codex-rate-card-draft";

test("edits rates in Credits per million tokens without floating point drift", () => {
  const draft = createCodexRateCardDraft(card);
  expect(draft.models[0]?.standard).toEqual({
    input: "125",
    cachedInput: "12.5",
    output: "750",
  });
  draft.models[0]!.standard.cachedInput = "1.875";

  const validation = validateCodexRateCardDraft(draft);
  expect(validation.errors).toEqual({});
  expect(validation.value?.models["gpt-5.6-sol"]?.standard.cached_input_nanos_per_million)
    .toBe(1_875_000_000);
  expect(rateCardContentEqual(validation.value!, card)).toBe(false);
});

test("validates duplicate models, tier relationships, and decimal precision", () => {
  const draft = createCodexRateCardDraft(card);
  draft.models.push({ ...draft.models[0]!, localId: "model-2" });
  draft.models[0]!.standard.cachedInput = "126";
  draft.models[0]!.standard.output = "1.0000000001";

  const validation = validateCodexRateCardDraft(draft);
  expect(validation.errors["model-2:name"]).toBe("模型名称不能重复");
  expect(validation.errors[tierFieldKey("model-1", "standard", "cachedInput")])
    .toBe("不能高于输入费率");
  expect(validation.errors[tierFieldKey("model-1", "standard", "output")])
    .toBe("最多输入 9 位小数");
});

test("generates a hidden replacement ID only for the submitted card", () => {
  const content = validateCodexRateCardDraft(createCodexRateCardDraft(card)).value!;
  expect(rateCardContentEqual(content, card)).toBe(true);
  expect(createVersionedRateCard(content, card.id, 123)).toEqual({
    ...card,
    id: "codex_rate_card_3f",
  });
});

const card: CodexRateCardValue = {
  id: "openai_codex_credits_2026_08_11",
  credits_per_usd: 25,
  models: {
    "gpt-5.6-sol": {
      standard: {
        input_nanos_per_million: 125_000_000_000,
        cached_input_nanos_per_million: 12_500_000_000,
        output_nanos_per_million: 750_000_000_000,
      },
    },
  },
};
