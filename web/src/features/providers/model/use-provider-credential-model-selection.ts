import { useMemo, useState } from "react";

import {
  MAX_UPSTREAM_MODEL_NAME_CHARS,
  type CredentialModelSelection,
} from "../api/provider-credential-contracts";

export function useProviderCredentialModelSelection(
  savedModels: readonly CredentialModelSelection[],
  discoveredModels: readonly string[],
) {
  const [selected, setSelected] = useState(
    () => new Set(savedModels.map((model) => model.upstreamModel)),
  );
  const [aliases, setAliases] = useState<ReadonlyMap<string, string>>(
    () =>
      new Map(
        savedModels
          .filter((model) => model.publicModel !== null)
          .map((model) => [model.upstreamModel, model.publicModel ?? ""]),
      ),
  );
  const [query, setQuery] = useState("");
  const [customModel, setCustomModelValue] = useState("");
  const [customError, setCustomError] = useState<string | undefined>();

  const savedUpstreamModels = useMemo(
    () => new Set(savedModels.map((model) => model.upstreamModel)),
    [savedModels],
  );

  const visibleModels = useMemo(() => {
    const values = new Set([...savedUpstreamModels, ...discoveredModels, ...selected]);
    const needle = query.trim().toLowerCase();
    return [...values]
      .filter((model) => !needle || model.toLowerCase().includes(needle))
      .sort((left, right) => left.localeCompare(right));
  }, [savedUpstreamModels, discoveredModels, query, selected]);

  const selectedEntries = useMemo<CredentialModelSelection[]>(
    () =>
      [...selected].sort().map((upstreamModel) => {
        const alias = aliases.get(upstreamModel)?.trim() ?? "";
        return {
          upstreamModel,
          publicModel: alias && alias !== upstreamModel ? alias : null,
        };
      }),
    [selected, aliases],
  );

  const selectionError = useMemo(() => {
    const publicNames = new Map<string, string>();
    for (const entry of selectedEntries) {
      const alias = aliases.get(entry.upstreamModel)?.trim();
      if (alias) {
        const validationError = validateModelName(alias, "公开名称");
        if (validationError) {
          return `「${entry.upstreamModel}」的${validationError}`;
        }
      }
      const publicModel = entry.publicModel ?? entry.upstreamModel;
      const existing = publicNames.get(publicModel);
      if (existing !== undefined) {
        return `公开名称「${publicModel}」同时来自「${existing}」和「${entry.upstreamModel}」，请修改其中一个`;
      }
      publicNames.set(publicModel, entry.upstreamModel);
    }
    return undefined;
  }, [selectedEntries, aliases]);

  function setCustomModel(value: string) {
    setCustomModelValue(value);
    setCustomError(undefined);
  }

  function addCustomModel() {
    const model = customModel.trim();
    const validationError = validateModelName(model, "模型名称");
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

  function setAlias(model: string, value: string) {
    setAliases((current) => {
      const next = new Map(current);
      if (value) {
        next.set(model, value);
      } else {
        next.delete(model);
      }
      return next;
    });
  }

  function aliasFor(model: string) {
    return aliases.get(model) ?? "";
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
    selectedEntries,
    selectionError,
    savedUpstreamModels,
    visibleModels,
    query,
    setQuery,
    customModel,
    setCustomModel,
    customError,
    addCustomModel,
    toggle,
    setAlias,
    aliasFor,
    selectVisible,
    clearVisible,
  };
}

function validateModelName(model: string, subject: string) {
  if (!model) {
    return `${subject}不能为空`;
  }
  if ([...model].length > MAX_UPSTREAM_MODEL_NAME_CHARS) {
    return `${subject}不能超过 ${MAX_UPSTREAM_MODEL_NAME_CHARS} 个字符`;
  }
  if ([...model].some(isControlCharacter)) {
    return `${subject}不能包含控制字符`;
  }
  return undefined;
}

function isControlCharacter(character: string) {
  const codePoint = character.codePointAt(0) ?? 0;
  return codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f);
}
