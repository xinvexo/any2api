import type {
  CodexRateCardValue,
  CodexRateTierValue,
} from "../api/settings-contracts";

const NANOS_PER_CREDIT = 1_000_000_000n;
const MAX_RATE_NANOS = 9_000_000_000_000_000n;
const MAX_CREDITS_PER_USD = 1_000_000;
const MAX_MODELS = 256;
const MAX_MODEL_NAME_CHARS = 255;

export interface CodexRateTierDraft {
  input: string;
  cachedInput: string;
  output: string;
}

export interface CodexRateModelDraft {
  localId: string;
  model: string;
  standard: CodexRateTierDraft;
  fastEnabled: boolean;
  fast: CodexRateTierDraft;
}

export interface CodexRateCardDraft {
  creditsPerUsd: string;
  models: CodexRateModelDraft[];
}

export interface CodexRateCardDraftValidation {
  value: Omit<CodexRateCardValue, "id"> | null;
  errors: Record<string, string>;
}

export function createCodexRateCardDraft(card: CodexRateCardValue): CodexRateCardDraft {
  return {
    creditsPerUsd: String(card.credits_per_usd),
    models: Object.entries(card.models).map(([model, rates], index) => ({
      localId: `model-${index + 1}`,
      model,
      standard: createTierDraft(rates.standard),
      fastEnabled: rates.fast !== null && rates.fast !== undefined,
      fast: createTierDraft(rates.fast ?? rates.standard),
    })),
  };
}

export function createEmptyCodexRateModelDraft(localId: string): CodexRateModelDraft {
  const empty = { input: "", cachedInput: "", output: "" };
  return {
    localId,
    model: "",
    standard: { ...empty },
    fastEnabled: false,
    fast: { ...empty },
  };
}

export function validateCodexRateCardDraft(
  draft: CodexRateCardDraft,
): CodexRateCardDraftValidation {
  const errors: Record<string, string> = {};
  const creditsPerUsd = parseCreditsPerUsd(draft.creditsPerUsd, errors);
  if (draft.models.length === 0) errors.models = "至少保留一个模型";
  if (draft.models.length > MAX_MODELS) errors.models = `模型不能超过 ${MAX_MODELS} 个`;

  const seenModels = new Set<string>();
  const models: CodexRateCardValue["models"] = {};
  for (const model of draft.models) {
    const name = model.model.trim();
    const nameKey = modelFieldKey(model.localId, "name");
    if (!name) {
      errors[nameKey] = "请输入模型名称";
    } else if ([...name].length > MAX_MODEL_NAME_CHARS) {
      errors[nameKey] = `不能超过 ${MAX_MODEL_NAME_CHARS} 个字符`;
    } else if (/\p{Cc}/u.test(name)) {
      errors[nameKey] = "不能包含控制字符";
    } else if (seenModels.has(name)) {
      errors[nameKey] = "模型名称不能重复";
    } else {
      seenModels.add(name);
    }

    const standard = parseTier(model, "standard", errors);
    const fast = model.fastEnabled ? parseTier(model, "fast", errors) : null;
    if (name && !errors[nameKey] && standard && (!model.fastEnabled || fast)) {
      models[name] = {
        standard,
        ...(fast ? { fast } : {}),
      };
    }
  }

  return {
    value: Object.keys(errors).length === 0 && creditsPerUsd !== null
      ? { credits_per_usd: creditsPerUsd, models }
      : null,
    errors,
  };
}

export function rateCardContentEqual(
  content: Omit<CodexRateCardValue, "id">,
  card: CodexRateCardValue,
) {
  if (content.credits_per_usd !== card.credits_per_usd) return false;
  const leftNames = Object.keys(content.models).sort();
  const rightNames = Object.keys(card.models).sort();
  return leftNames.length === rightNames.length
    && leftNames.every((name, index) => {
      if (name !== rightNames[index]) return false;
      const left = content.models[name];
      const right = card.models[name];
      return Boolean(left && right)
        && tierEqual(left!.standard, right!.standard)
        && optionalTierEqual(left!.fast, right!.fast);
    });
}

export function createVersionedRateCard(
  content: Omit<CodexRateCardValue, "id">,
  currentId: string,
  timestamp = Date.now(),
): CodexRateCardValue {
  const base = `codex_rate_card_${timestamp.toString(36)}`;
  const id = base === currentId ? `${base}_1` : base;
  return {
    id: id === currentId ? `${base}_2` : id,
    ...content,
  };
}

export function modelFieldKey(localId: string, field: "name") {
  return `${localId}:${field}`;
}

export function tierFieldKey(
  localId: string,
  tier: "standard" | "fast",
  field: keyof CodexRateTierDraft,
) {
  return `${localId}:${tier}:${field}`;
}

function createTierDraft(tier: CodexRateTierValue): CodexRateTierDraft {
  return {
    input: nanosToCredits(tier.input_nanos_per_million),
    cachedInput: nanosToCredits(tier.cached_input_nanos_per_million),
    output: nanosToCredits(tier.output_nanos_per_million),
  };
}

function parseCreditsPerUsd(value: string, errors: Record<string, string>) {
  const text = value.trim();
  if (!/^\d+$/u.test(text)) {
    errors.creditsPerUsd = "请输入正整数";
    return null;
  }
  const number = Number(text);
  if (!Number.isSafeInteger(number) || number < 1 || number > MAX_CREDITS_PER_USD) {
    errors.creditsPerUsd = `请输入 1 到 ${MAX_CREDITS_PER_USD} 之间的整数`;
    return null;
  }
  return number;
}

function parseTier(
  model: CodexRateModelDraft,
  tierName: "standard" | "fast",
  errors: Record<string, string>,
): CodexRateTierValue | null {
  const tier = model[tierName];
  const input = parseRate(tier.input, true, tierFieldKey(model.localId, tierName, "input"), errors);
  const cachedInput = parseRate(
    tier.cachedInput,
    false,
    tierFieldKey(model.localId, tierName, "cachedInput"),
    errors,
  );
  const output = parseRate(tier.output, true, tierFieldKey(model.localId, tierName, "output"), errors);
  if (input !== null && cachedInput !== null && cachedInput > input) {
    errors[tierFieldKey(model.localId, tierName, "cachedInput")] = "不能高于输入费率";
  }
  return input !== null && cachedInput !== null && output !== null && cachedInput <= input
    ? {
        input_nanos_per_million: input,
        cached_input_nanos_per_million: cachedInput,
        output_nanos_per_million: output,
      }
    : null;
}

function parseRate(
  value: string,
  positive: boolean,
  key: string,
  errors: Record<string, string>,
) {
  const text = value.trim();
  const match = /^(\d+)(?:\.(\d{1,9}))?$/u.exec(text);
  if (!match) {
    errors[key] = "最多输入 9 位小数";
    return null;
  }
  const whole = BigInt(match[1]!);
  const fraction = BigInt((match[2] ?? "").padEnd(9, "0") || "0");
  const nanos = whole * NANOS_PER_CREDIT + fraction;
  if ((positive && nanos === 0n) || nanos > MAX_RATE_NANOS) {
    errors[key] = positive ? "请输入有效的正数费率" : "请输入有效的非负费率";
    return null;
  }
  return Number(nanos);
}

function nanosToCredits(value: number) {
  const nanos = BigInt(value);
  const whole = nanos / NANOS_PER_CREDIT;
  const fraction = (nanos % NANOS_PER_CREDIT).toString().padStart(9, "0").replace(/0+$/u, "");
  return fraction ? `${whole}.${fraction}` : whole.toString();
}

function tierEqual(left: CodexRateTierValue, right: CodexRateTierValue) {
  return left.input_nanos_per_million === right.input_nanos_per_million
    && left.cached_input_nanos_per_million === right.cached_input_nanos_per_million
    && left.output_nanos_per_million === right.output_nanos_per_million;
}

function optionalTierEqual(
  left: CodexRateTierValue | null | undefined,
  right: CodexRateTierValue | null | undefined,
) {
  if (!left || !right) return !left && !right;
  return tierEqual(left, right);
}
