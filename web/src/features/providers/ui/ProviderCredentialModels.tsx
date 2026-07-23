import { Check, RefreshCw, Search, X } from "lucide-react";
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
  const status = result ? describeResult(result) : null;

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

      {status ? (
        <p
          className={status.tone === "danger" ? "text-sm text-danger" : "text-sm text-warning"}
          role={status.tone === "danger" ? "alert" : "status"}
        >
          {status.message}
        </p>
      ) : null}

      {result && result.accepted && result.catalogValid ? (
        <div className="flex flex-wrap items-center justify-between gap-2 text-[12px] text-secondary">
          <span>已读取 {discovered.length} 个模型 · 已选择 {selected.size} 个</span>
          <div className="flex flex-wrap items-center gap-1">
            <Button variant="ghost" size="sm" onClick={onDiscover} disabled={pending}>
              <RefreshCw size={14} className={pending ? "animate-spin" : undefined} />
              重新拉取
            </Button>
            <Button variant="ghost" size="sm" onClick={selectVisible} disabled={pending || models.length === 0}>
              <Check size={14} />
              全选当前
            </Button>
            <Button variant="ghost" size="sm" onClick={clearVisible} disabled={pending || models.length === 0}>
              <X size={14} />
              清除当前
            </Button>
          </div>
        </div>
      ) : null}

      {result && !(result.accepted && result.catalogValid) ? (
        <div className="flex justify-end">
          <Button variant="ghost" size="sm" onClick={onDiscover} disabled={pending}>
            <RefreshCw size={14} className={pending ? "animate-spin" : undefined} />
            重新拉取
          </Button>
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
              <p className="p-6 text-center text-sm text-secondary">
                {query.trim() ? "没有匹配的模型" : "上游没有返回可用模型"}
              </p>
            ) : (
              <div className="divide-y divide-subtle">
                {models.map((model) => {
                  const saved = credential.models.includes(model);
                  const returned = discovered.includes(model);
                  return (
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
                      <span className="min-w-0 break-all font-mono text-[12px]">{model}</span>
                      {saved && !returned ? (
                        <span className="ml-auto shrink-0 text-[11px] text-warning">已保存</span>
                      ) : null}
                    </label>
                  );
                })}
              </div>
            )}
          </div>
        </>
      ) : null}

      {error ? <FormError>{getProviderErrorMessage(error)}</FormError> : null}

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          关闭
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={pending || !result?.accepted || !result.catalogValid}
          onClick={() => void onSave([...selected].sort())}
        >
          保存
        </Button>
      </div>
    </div>
  );
}

function describeResult(result: ProviderCredentialTestResult) {
  if (!result.reachable) {
    const scope = describeFailureScope(result.failureScope);
    const stage = describeFailureStage(result.errorStage);
    return {
      tone: "danger" as const,
      message: `无法通过${scope}连接上游${stage ? `（${stage}失败）` : ""}。`,
    };
  }
  if (!result.accepted) {
    return {
      tone: "danger" as const,
      message: `上游拒绝了这把 API Key${result.statusCode ? `（HTTP ${result.statusCode}）` : ""}。`,
    };
  }
  if (!result.catalogValid) {
    return {
      tone: "danger" as const,
      message: "上游返回的模型目录无法识别，请重新拉取。",
    };
  }
  if (result.models.length === 0) {
    return {
      tone: "warning" as const,
      message: "上游返回了空模型列表。",
    };
  }
  return null;
}

function describeFailureScope(scope: string | null) {
  switch (scope) {
    case "endpoint":
      return "上游地址";
    case "proxy":
      return "出口代理";
    default:
      return "网络链路";
  }
}

function describeFailureStage(stage: string | null) {
  switch (stage) {
    case "dns":
      return "解析";
    case "tcp":
      return "连接";
    case "proxy_handshake":
      return "出口代理握手";
    case "tls":
      return "TLS";
    case "write_request":
      return "发送请求";
    case "await_headers":
      return "等待响应";
    case "read_body":
      return "读取响应";
    default:
      return null;
  }
}
