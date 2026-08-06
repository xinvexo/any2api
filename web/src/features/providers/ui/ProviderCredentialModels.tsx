import { Check, Plus, RefreshCw, Search, X } from "lucide-react";
import { useEffect, useRef } from "react";

import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderCredentialModelSelection } from "../model/use-provider-credential-model-selection";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError, FormNotice } from "@/shared/ui/form-field";

const EMPTY_MODELS: readonly string[] = [];

export function ProviderCredentialModels({
  credential,
  result,
  discovering,
  saving,
  error,
  onDiscover,
  onSave,
  onClose,
}: {
  credential: ProviderCredential;
  result: ProviderCredentialTestResult | undefined;
  discovering: boolean;
  saving: boolean;
  error: unknown;
  onDiscover: (manual?: boolean) => void;
  onSave: (models: string[]) => Promise<void>;
  onClose: () => void;
}) {
  const discovered = result?.models ?? EMPTY_MODELS;
  const selection = useProviderCredentialModelSelection(credential.models, discovered);
  const requested = useRef(false);

  useEffect(() => {
    if (!requested.current) {
      requested.current = true;
      onDiscover(false);
    }
  }, [onDiscover]);

  const status = result ? describeResult(result) : null;
  const customModelId = `custom-model-${credential.id}`;

  return (
    <div className="space-y-5">
      {discovering ? (
        <p className="flex items-center gap-2 text-sm text-secondary" aria-busy="true">
          <RefreshCw size={15} className="animate-spin" />
          正在读取上游模型
        </p>
      ) : null}

      {status ? <FormNotice tone={status.tone}>{status.message}</FormNotice> : null}

      <div className="flex flex-wrap items-center justify-between gap-2 text-[12px] text-secondary">
        <span>
          {result?.accepted && result.catalogValid ? `已读取 ${discovered.length} 个模型 · ` : ""}
          已选择 {selection.selected.size} 个
        </span>
        <div className="flex flex-wrap items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onDiscover(true)}
            disabled={discovering || saving}
          >
            <RefreshCw size={14} className={discovering ? "animate-spin" : undefined} />
            重新拉取
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={selection.selectVisible}
            disabled={saving || selection.visibleModels.length === 0}
          >
            <Check size={14} />
            全选当前
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={selection.clearVisible}
            disabled={saving || selection.visibleModels.length === 0}
          >
            <X size={14} />
            清除当前
          </Button>
        </div>
      </div>

      <Field
        label="手动添加模型"
        htmlFor={customModelId}
        error={selection.customError}
        hint="填写上游实际模型名；公开名称保持一致，不会创建别名。"
      >
        <div className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2">
          <input
            id={customModelId}
            className={controlClass()}
            value={selection.customModel}
            placeholder="例如 gpt-5.6-sol"
            autoComplete="off"
            disabled={saving}
            aria-describedby={selection.customError ? `${customModelId}-error` : undefined}
            onChange={(event) => selection.setCustomModel(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                selection.addCustomModel();
              }
            }}
          />
          <Button
            variant="secondary"
            onClick={selection.addCustomModel}
            disabled={saving || !selection.customModel.trim()}
          >
            <Plus size={14} />
            添加
          </Button>
        </div>
      </Field>

      <div className="relative">
        <Search
          size={14}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-tertiary"
          aria-hidden="true"
        />
        <input
          className={`${controlClass()} pl-9`}
          value={selection.query}
          placeholder="搜索已发现或已添加的模型"
          aria-label="搜索模型"
          disabled={saving}
          onChange={(event) => selection.setQuery(event.target.value)}
        />
      </div>
      <div className="max-h-[min(52vh,28rem)] overflow-y-auto rounded-[8px] border border-subtle">
        {selection.visibleModels.length === 0 ? (
          <p className="p-6 text-center text-sm text-secondary">
            {selection.query.trim() ? "没有匹配的模型" : "尚未发现或添加模型"}
          </p>
        ) : (
          <div className="divide-y divide-subtle">
            {selection.visibleModels.map((model) => {
              const saved = credential.models.includes(model);
              const returned = discovered.includes(model);
              const manuallyAdded = selection.selected.has(model) && !saved && !returned;
              return (
                <label
                  key={model}
                  className="flex cursor-pointer items-center gap-3 px-3 py-3 text-sm hover:bg-surface-hover"
                >
                  <input
                    type="checkbox"
                    className="size-4 accent-accent"
                    aria-label={model}
                    checked={selection.selected.has(model)}
                    disabled={saving}
                    onChange={() => selection.toggle(model)}
                  />
                  <span className="min-w-0 break-all font-mono text-[12px]">{model}</span>
                  {saved && !returned ? (
                    <span className="ml-auto shrink-0 text-[11px] text-warning">已保存</span>
                  ) : manuallyAdded ? (
                    <span className="ml-auto shrink-0 text-[11px] text-secondary">手动</span>
                  ) : null}
                </label>
              );
            })}
          </div>
        )}
      </div>

      {error ? <FormError>{getProviderErrorMessage(error)}</FormError> : null}

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={saving} onClick={onClose}>
          关闭
        </Button>
        <Button
          type="button"
          variant="primary"
          disabled={saving}
          onClick={() => void onSave(selection.selectedModels)}
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
    if (result.statusCode === 401 || result.statusCode === 403) {
      return {
        tone: "danger" as const,
        message: `模型目录请求被上游拒绝（HTTP ${result.statusCode}）；请核对 Base URL 与上游认证要求，也可手动添加模型。`,
      };
    }
    return {
      tone: "danger" as const,
      message: `读取上游模型目录失败${result.statusCode ? `（HTTP ${result.statusCode}）` : ""}，可手动添加已确认的模型。`,
    };
  }
  if (!result.catalogValid) {
    return {
      tone: "danger" as const,
      message: "上游返回的模型目录无法识别，可手动添加已确认的模型。",
    };
  }
  if (result.models.length === 0) {
    return {
      tone: "warning" as const,
      message: "上游返回了空模型列表，可手动添加已确认的模型。",
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
    case "egress_path":
      return "上游地址与出口代理组合";
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
