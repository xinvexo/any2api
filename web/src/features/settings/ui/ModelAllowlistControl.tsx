import { Check, Search, X } from "lucide-react";
import { useMemo, useState } from "react";

import type { SettingItem } from "../api/settings-contracts";
import type { ModelAccessDraft, SettingDraft } from "../model/setting-draft";
import { cn } from "@/shared/lib/cn";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Switch } from "@/shared/ui/Switch";

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
  const access: ModelAccessDraft =
    typeof value === "object" ? value : { mode: "all", models: [] };
  const selected = useMemo(() => new Set(access.models), [access.models]);
  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return options.filter((model) => !needle || model.toLowerCase().includes(needle));
  }, [options, query]);

  function publish(next: Set<string>) {
    onChange({
      mode: next.size === 0 ? "all" : "only",
      models: [...next].sort(),
    });
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
          {access.mode === "all" || selected.size === 0
            ? `全部 ${options.length} 个模型可用`
            : `已允许 ${selected.size} / ${options.length}`}
        </span>
        <div className="flex items-center gap-2 text-[12px] text-secondary">
          <span>允许全部</span>
          <Switch
            checked={access.mode === "all"}
            disabled={disabled}
            aria-label="允许全部公开模型"
            onCheckedChange={(checked) => {
              if (checked) {
                onChange({ mode: "all", models: access.models });
                return;
              }
              publish(new Set(access.models.length > 0 ? access.models : options));
            }}
          />
        </div>
      </div>
      <div className="flex flex-wrap items-center justify-end gap-1">
        <Button
          variant="ghost"
          size="sm"
          disabled={disabled || access.mode === "all" || visible.length === 0}
          onClick={selectVisible}
        >
          <Check size={14} aria-hidden="true" />
          选择当前
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={disabled || access.mode === "all" || visible.length === 0}
          onClick={clearVisible}
        >
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
          className={controlClass(false, "pl-9")}
          value={query}
          placeholder="搜索模型"
          aria-label="搜索可用模型"
          aria-describedby={describedBy}
          disabled={disabled || access.mode === "all"}
          onChange={(event) => setQuery(event.target.value)}
        />
      </div>

      <div
        className="max-h-64 min-h-12 overflow-y-auto"
        role="group"
        aria-label="可用模型"
      >
        {visible.length === 0 ? (
          <p className="rounded-[8px] bg-surface-muted px-2 py-4 text-center text-[12px] text-secondary">
            {query.trim() ? "没有匹配的模型" : "暂无已发布模型"}
          </p>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(11rem,1fr))] gap-1.5 p-0.5">
            {visible.map((model) => {
              const checked = selected.has(model);
              return (
                <button
                  key={model}
                  type="button"
                  className={cn(
                    "focus-ring flex h-9 min-w-0 items-center gap-2 rounded-[7px] border px-2.5 text-left transition-colors",
                    "disabled:cursor-not-allowed disabled:opacity-45",
                    checked
                      ? "border-accent/40 bg-accent/10 text-accent-copy hover:bg-accent/15"
                      : "border-subtle bg-surface text-primary hover:border-strong hover:bg-surface-hover",
                  )}
                  aria-label={model}
                  aria-pressed={checked}
                  title={model}
                  disabled={disabled || access.mode === "all"}
                  onClick={() => toggle(model)}
                >
                  <Check
                    size={14}
                    className={cn("shrink-0", checked ? "opacity-100" : "opacity-0")}
                    aria-hidden="true"
                  />
                  <span className="min-w-0 flex-1 truncate font-mono text-[12px]">
                    {model}
                  </span>
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
