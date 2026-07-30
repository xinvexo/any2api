import { useMemo, useState } from "react";

import { MAX_UPSTREAM_MODEL_NAME_CHARS } from "../api/provider-credential-contracts";

export function useProviderCredentialModelSelection(
  savedModels: readonly string[],
  discoveredModels: readonly string[],
) {
  const [selected, setSelected] = useState(() => new Set(savedModels));
  const [query, setQuery] = useState("");
  const [customModel, setCustomModelValue] = useState("");
  const [customError, setCustomError] = useState<string | undefined>();

  const visibleModels = useMemo(() => {
    const values = new Set([...savedModels, ...discoveredModels, ...selected]);
    const needle = query.trim().toLowerCase();
    return [...values]
      .filter((model) => !needle || model.toLowerCase().includes(needle))
      .sort((left, right) => left.localeCompare(right));
  }, [savedModels, discoveredModels, query, selected]);

  function setCustomModel(value: string) {
    setCustomModelValue(value);
    setCustomError(undefined);
  }

  function addCustomModel() {
    const model = customModel.trim();
    const validationError = validateModelName(model);
    if (validationError) {
      setCustomError(validationError);
      return;
    }
    setSelected((current) => new Set([...current, model]));
    setCustomModelValue("");
    setCustomError(undefined);
  }

  function toggle(model: string) {
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(model)) {
        next.delete(model);
      } else {
        next.add(model);
      }
      return next;
    });
  }

  function selectVisible() {
    setSelected((current) => new Set([...current, ...visibleModels]));
  }

  function clearVisible() {
    setSelected((current) => {
      const next = new Set(current);
      visibleModels.forEach((model) => next.delete(model));
      return next;
    });
  }

  return {
    selected,
    selectedModels: [...selected].sort(),
    visibleModels,
    query,
    setQuery,
    customModel,
    setCustomModel,
    customError,
    addCustomModel,
    toggle,
    selectVisible,
    clearVisible,
  };
}

function validateModelName(model: string) {
  if (!model) {
    return "模型名称不能为空";
  }
  if ([...model].length > MAX_UPSTREAM_MODEL_NAME_CHARS) {
    return `模型名称不能超过 ${MAX_UPSTREAM_MODEL_NAME_CHARS} 个字符`;
  }
  if ([...model].some(isControlCharacter)) {
    return "模型名称不能包含控制字符";
  }
  return undefined;
}

function isControlCharacter(character: string) {
  const codePoint = character.codePointAt(0) ?? 0;
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}
