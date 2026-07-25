import { Check, Search, X } from "lucide-react";
import { useMemo, useState } from "react";

import type { SettingItem } from "../api/settings-contracts";
import type { SettingDraft } from "../model/setting-draft";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";

export function ModelAllowlistControl({
  item,
  value,
  disabled,
  labelledBy,
  describedBy,
  onChange,
}: {
  item: SettingItem;
  value: SettingDraft;
  disabled: boolean;
  labelledBy: string;
  describedBy: string;
  onChange: (value: SettingDraft) => void;
}) {
  const [query, setQuery] = useState("");
  const options = useMemo(() => item.options ?? [], [item.options]);
  const selected = useMemo(
    () => new Set(Array.isArray(value) ? value : []),
    [value],
  );
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return options.filter((model) => !needle || model.toLowerCase().includes(needle));
  }, [options, query]);

  function publish(next: Set<string>) {
    onChange([...next].sort());
  }

  function toggle(model: string) {
    const next = new Set(selected);
    if (next.has(model)) {
      next.delete(model);
    } else {
      next.add(model);
    }
    publish(next);
  }

  function selectVisible() {
    publish(new Set([...selected, ...visible]));
  }

  function clearVisible() {
    const next = new Set(selected);
    visible.forEach((model) => next.delete(model));
    publish(next);
  }

  return (
    <div className="min-w-0 space-y-2.5" role="group" aria-labelledby={labelledBy}>
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-[12px] tabular-nums text-secondary">
          {selected.size === 0 ? `全部 ${options.length} 个模型可用` : `已允许 ${selected.size} / ${options.length}`}
        </span>
        <div className="flex flex-wrap items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            disabled={disabled || visible.length === 0}
            onClick={selectVisible}
          >
            <Check size={14} aria-hidden="true" />
            选择当前
          </Button>
          <Button
            variant="ghost"
            size="sm"
            disabled={disabled || visible.length === 0}
            onClick={clearVisible}
          >
            <X size={14} aria-hidden="true" />
            清除当前
          </Button>
        </div>
      </div>

      <div className="relative">
        <Search
          size={14}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-tertiary"
          aria-hidden="true"
        />
        <input
          className={controlClass(false, "pl-9")}
          value={query}
          placeholder="搜索模型"
          aria-label="搜索可用模型"
          aria-describedby={describedBy}
          disabled={disabled}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>

      <div className="max-h-64 min-h-12 overflow-y-auto border-y border-subtle" aria-label="可用模型">
        {visible.length === 0 ? (
          <p className="px-2 py-4 text-center text-[12px] text-secondary">
            {query.trim() ? "没有匹配的模型" : "暂无已发布模型"}
          </p>
        ) : (
          <div className="divide-y divide-subtle">
            {visible.map((model) => (
              <label
                key={model}
                className="flex cursor-pointer items-center gap-3 px-2 py-2.5 hover:bg-surface-hover"
              >
                <input
                  type="checkbox"
                  className="size-4 accent-accent"
                  checked={selected.has(model)}
                  disabled={disabled}
                  aria-label={model}
                  onChange={() => toggle(model)}
                />
                <span className="min-w-0 flex-1 break-all font-mono text-[12px] text-primary">
                  {model}
                </span>
              </label>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
