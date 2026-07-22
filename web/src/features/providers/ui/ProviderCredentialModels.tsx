import { Check, RefreshCw, Save, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { FormError } from "@/shared/ui/form-field";

const EMPTY_MODELS: readonly string[] = [];

export function ProviderCredentialModels({
  credential,
  result,
  pending,
  error,
  onDiscover,
  onSave,
  onClose,
}: {
  credential: ProviderCredential;
  result: ProviderCredentialTestResult | undefined;
  pending: boolean;
  error: unknown;
  onDiscover: () => void;
  onSave: (models: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const discovered = result?.models ?? EMPTY_MODELS;
  const [selected, setSelected] = useState(() => new Set(credential.models));
  const [query, setQuery] = useState("");
  const requested = useRef(false);

  useEffect(() => {
    if (!requested.current) {
      requested.current = true;
      onDiscover();
    }
  }, [onDiscover]);

  const models = useMemo(() => {
    const values = new Set([...credential.models, ...discovered]);
    const needle = query.trim().toLowerCase();
    return [...values]
      .filter((model) => !needle || model.toLowerCase().includes(needle))
      .sort((left, right) => left.localeCompare(right));
  }, [credential.models, discovered, query]);

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
    setSelected((current) => new Set([...current, ...models]));
  }

  function clearVisible() {
    setSelected((current) => {
      const next = new Set(current);
      models.forEach((model) => next.delete(model));
      return next;
    });
  }

  const discovering = pending && result === undefined;
  const catalogError = result && !result.catalogValid
    ? "上游返回了无法识别的模型目录，请重新拉取。"
    : null;

  return (
    <div className="space-y-5">
      <div className="rounded-[8px] bg-surface-muted px-3 py-3 text-[13px] text-secondary">
        从这把 API Key 实际能访问的上游模型中选择。只有选中的模型会出现在公开模型列表并参与调度。
      </div>

      {discovering ? (
        <div className="flex min-h-28 items-center justify-center gap-2 text-sm text-secondary" aria-busy="true">
          <RefreshCw size={15} className="animate-spin" />
          正在读取上游模型
        </div>
      ) : null}

      {result && result.accepted && result.catalogValid ? (
        <div className="flex flex-wrap items-center justify-between gap-2 text-[12px] text-secondary">
          <span>已读取 {discovered.length} 个模型 · 已选择 {selected.size} 个</span>
          <div className="flex gap-1">
            <Button variant="ghost" onClick={selectVisible} disabled={pending || models.length === 0}>
              <Check size={14} />
              全选当前
            </Button>
            <Button variant="ghost" onClick={clearVisible} disabled={pending || models.length === 0}>
              <X size={14} />
              清除当前
            </Button>
          </div>
        </div>
      ) : null}

      {result && result.accepted && result.catalogValid ? (
        <>
          <div className="relative">
            <Search
              size={14}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-tertiary"
              aria-hidden="true"
            />
            <input
              className={`${controlClass()} pl-9`}
              value={query}
              placeholder="搜索模型"
              aria-label="搜索模型"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          <div className="max-h-[min(52vh,28rem)] overflow-y-auto rounded-[8px] border border-subtle">
            {models.length === 0 ? (
              <p className="p-6 text-center text-sm text-secondary">没有匹配的模型</p>
            ) : (
              <div className="divide-y divide-subtle">
                {models.map((model) => (
                  <label key={model} className="flex cursor-pointer items-center gap-3 px-3 py-3 text-sm hover:bg-surface-hover">
                    <input
                      type="checkbox"
                      className="size-4 accent-accent"
                      checked={selected.has(model)}
                      disabled={pending}
                      onChange={() => toggle(model)}
                    />
                    <span className="min-w-0 break-all font-mono text-[12px]">{model}</span>
                  </label>
                ))}
              </div>
            )}
          </div>
        </>
      ) : null}

      {catalogError ? <p className="text-sm text-danger" role="alert">{catalogError}</p> : null}
      {error ? <FormError>{getProviderErrorMessage(error)}</FormError> : null}

      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          关闭
        </Button>
        <Button type="button" variant="ghost" disabled={pending} onClick={onDiscover}>
          <RefreshCw size={14} className={discovering ? "animate-spin" : undefined} />
          重新拉取
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={pending || !result?.accepted || !result.catalogValid}
          onClick={() => void onSave([...selected].sort())}
        >
          <Save size={14} />
          保存模型
        </Button>
      </div>
    </div>
  );
}
