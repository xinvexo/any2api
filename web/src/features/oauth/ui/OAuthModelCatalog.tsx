import { Check, Search, X } from "lucide-react";
import { useMemo, useState, type FormEvent } from "react";

import type { OAuthAccount } from "../api/oauth-contracts";
import { presentOAuthAccount } from "../model/oauth-account-presentation";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { FormError } from "@/shared/ui/form-field";

export function OAuthModelCatalog({
  account,
  pending,
  error,
  onSave,
  onClose,
}: {
  account: OAuthAccount;
  pending: boolean;
  error: unknown;
  onSave: (models: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const presentation = presentOAuthAccount(account);
  const [selected, setSelected] = useState(() => new Set(account.models));
  const [query, setQuery] = useState("");
  const catalog = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return presentation.modelCatalog.filter(
      (model) => !needle || model.toLowerCase().includes(needle),
    );
  }, [presentation.modelCatalog, query]);
  const changed = !sameModels(selected, account.models);

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
    setSelected((current) => new Set([...current, ...catalog]));
  }

  function clearVisible() {
    setSelected((current) => {
      const next = new Set(current);
      catalog.forEach((model) => next.delete(model));
      return next;
    });
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      await onSave([...selected].sort((left, right) => left.localeCompare(right)));
      onClose();
    } catch {
      // Keep the current selection visible after a revision conflict or validation error.
    }
  }

  return (
    <form className="flex min-h-0 flex-col gap-5" onSubmit={(event) => void submit(event)}>
      <header className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="truncate text-[15px] font-semibold tracking-tight text-primary">
            {presentation.title}
          </p>
          <p className="mt-1 truncate text-[12px] text-secondary">{presentation.subtitle}</p>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-1.5">
          {presentation.badges
            .filter((badge) => badge.tone === "neutral")
            .map((badge) => (
              <span
                key={badge.key}
                className="rounded-full bg-surface-muted px-2 py-0.5 text-[11px] font-medium text-secondary"
              >
                {badge.label}
              </span>
            ))}
          <span className="tabular-nums text-[11px] text-tertiary">
            已选择 {selected.size} / {presentation.modelCatalog.length}
          </span>
        </div>
      </header>

      <p className="text-[12px] leading-5 text-secondary">
        只有选中的模型会出现在公开模型列表并参与路由。清空选择会停用该账号的全部模型路由。
      </p>

      <div className="flex flex-wrap items-center justify-end gap-1">
        <Button type="button" variant="ghost" size="sm" disabled={pending || catalog.length === 0} onClick={selectVisible}>
          <Check size={14} aria-hidden="true" />
          全选当前
        </Button>
        <Button type="button" variant="ghost" size="sm" disabled={pending || catalog.length === 0} onClick={clearVisible}>
          <X size={14} aria-hidden="true" />
          清除当前
        </Button>
      </div>

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
          disabled={pending}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>

      <div className="max-h-[min(52vh,28rem)] overflow-y-auto rounded-[8px] border border-subtle">
        {catalog.length === 0 ? (
          <p className="p-6 text-center text-[13px] text-secondary">
            {query.trim() ? "没有匹配的模型" : "该账号模型目录为空"}
          </p>
        ) : (
          <div className="divide-y divide-subtle" aria-label="可用模型">
            {catalog.map((model) => (
              <label
                key={model}
                className="flex cursor-pointer items-center gap-3 px-3 py-3 text-sm hover:bg-surface-hover"
              >
                <input
                  type="checkbox"
                  className="size-4 accent-accent"
                  aria-label={model}
                  checked={selected.has(model)}
                  disabled={pending}
                  onChange={() => toggle(model)}
                />
                <span className="min-w-0 flex-1 break-all font-mono text-[12px]">{model}</span>
              </label>
            ))}
          </div>
        )}
      </div>

      {error ? <FormError>{getOAuthErrorMessage(error)}</FormError> : null}

      <div className="flex justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" disabled={pending} onClick={onClose}>
          关闭
        </Button>
        <Button type="submit" variant="primary" disabled={pending || !changed}>
          保存
        </Button>
      </div>
    </form>
  );
}

function sameModels(selected: Set<string>, saved: readonly string[]) {
  return selected.size === saved.length && saved.every((model) => selected.has(model));
}
