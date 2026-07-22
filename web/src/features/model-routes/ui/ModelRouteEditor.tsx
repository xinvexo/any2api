import { Plus, Route, Save, X } from "lucide-react";
import { useEffect, useRef, type FormEvent, type ReactNode } from "react";

import type { ProtocolDialect, ProviderEndpoint } from "@/features/providers";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

import type { ModelRoute, ModelRouteWriteInput } from "../api/model-route-contracts";
import { getModelRouteErrorMessage } from "../model/model-route-error";
import { useModelRouteEditor } from "../model/use-model-route-editor";
import { RouteTargetFields } from "./RouteTargetFields";

interface ModelRouteEditorProps {
  route?: ModelRoute;
  endpoints: ProviderEndpoint[];
  editing: boolean;
  sourceConflict: "changed" | "deleted" | null;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ModelRouteWriteInput) => Promise<void>;
  onClose: () => void;
}

export function ModelRouteEditor({
  route,
  endpoints,
  editing,
  sourceConflict,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: ModelRouteEditorProps) {
  const editor = useModelRouteEditor(endpoints, route);
  const publicModelRef = useRef<HTMLInputElement>(null);
  const compatibleEndpointCount = endpoints.filter(
    (endpoint) => endpoint.protocolDialect === editor.draft.ingressProtocol,
  ).length;

  useEffect(() => {
    publicModelRef.current?.focus();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sourceConflict) {
      return;
    }
    const input = editor.buildInput(configRevision);
    if (!input) {
      return;
    }
    try {
      await onSubmit(input);
      onClose();
    } catch {
      // Keep the aggregate draft visible after a revision or validation error.
    }
  }

  return (
    <Surface className="h-fit overflow-hidden xl:sticky xl:top-24">
      <div className="flex items-start justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div className="flex min-w-0 gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-control bg-surface-muted text-accent-copy">
            <Route size={17} aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h2 className="font-semibold">{editing ? "编辑模型路由" : "新增模型路由"}</h2>
            <p className="mt-1 text-sm text-secondary">一次保存完整 Route 与 Target 集合</p>
          </div>
        </div>
        <Button
          variant="ghost"
          className="size-9 shrink-0 px-0"
          onClick={onClose}
          disabled={pending}
          aria-label="关闭编辑器"
          title="关闭编辑器"
        >
          <X size={17} />
        </Button>
      </div>

      <form className="space-y-5 p-5 sm:p-6" onSubmit={(event) => void submit(event)}>
        {sourceConflict ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-warning" role="status">
            {sourceConflict === "deleted"
              ? "此模型路由已从最新配置中删除；草稿仍保留，请复制需要的内容后关闭。"
              : "此模型路由已被其他操作修改；草稿仍保留，请关闭后重新打开。"}
          </p>
        ) : null}
<Field label="公开模型名" error={editor.errors.publicModel} htmlFor="route-public-model">
          <input
            id="route-public-model"
            ref={publicModelRef}
            className={inputClass}
            value={editor.draft.publicModel}
            maxLength={255}
            required
            disabled={pending}
            autoComplete="off"
            spellCheck={false}
            aria-invalid={Boolean(editor.errors.publicModel)}
            aria-describedby={
              editor.errors.publicModel ? "route-public-model-error" : undefined
            }
            onChange={(event) => editor.update("publicModel", event.target.value)}
          />
        </Field>

        <div className="grid gap-4 sm:grid-cols-2">
          <Field label="入口协议" htmlFor="route-ingress-protocol">
            <select
              id="route-ingress-protocol"
              className={inputClass}
              value={editor.draft.ingressProtocol}
              disabled={pending || editing}
              onChange={(event) =>
                editor.updateProtocol(readProtocolDialect(event.target.value))
              }
            >
              <option value="openai_responses">OpenAI Responses</option>
              <option value="anthropic_messages">Anthropic Messages</option>
            </select>
          </Field>

          <Field label="主 tier 满载" htmlFor="route-fallback-policy">
            <select
              id="route-fallback-policy"
              className={inputClass}
              value={editor.draft.fallbackOnSaturation}
              disabled={pending}
              onChange={(event) =>
                editor.update(
                  "fallbackOnSaturation",
                  event.target.value === "fallback"
                    ? "fallback"
                    : event.target.value === "wait"
                      ? "wait"
                      : "inherit",
                )
              }
            >
              <option value="inherit">继承全局设置</option>
              <option value="wait">停留当前 tier</option>
              <option value="fallback">进入下一 tier</option>
            </select>
          </Field>
        </div>

        <div className="flex items-start gap-3 rounded-control border border-subtle bg-surface-muted px-4 py-3">
          <input
            id="route-enabled"
            type="checkbox"
            className="mt-0.5 size-4 accent-accent"
            checked={editor.draft.enabled}
            disabled={pending}
            onChange={(event) => editor.update("enabled", event.target.checked)}
          />
          <span>
            <label htmlFor="route-enabled" className="block text-sm font-medium">
              启用此模型路由
            </label>
            <span className="mt-1 block text-xs leading-5 text-secondary">
              停用后配置仍会保留，但不会出现在公开模型列表中。
            </span>
          </span>
        </div>

        <section aria-labelledby="route-targets-heading">
          <div className="flex flex-col gap-3 border-b border-subtle pb-4 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h3 id="route-targets-heading" className="text-sm font-semibold">Route Targets</h3>
              <p className="mt-1 text-xs text-tertiary">
                {compatibleEndpointCount} 个同协议 Endpoint
              </p>
            </div>
            <Button type="button" variant="ghost" disabled={pending} onClick={editor.addTarget}>
              <Plus size={15} />
              添加 Target
            </Button>
          </div>

          {editor.errors.targets ? (
            <p className="mt-3 text-sm text-danger" role="alert">{editor.errors.targets}</p>
          ) : null}

          <div className="pt-5">
            {editor.draft.targets.map((target, index) => (
              <RouteTargetFields
                key={target.clientId}
                index={index}
                target={target}
                errors={editor.errors.targetByClientId[target.clientId]}
                endpoints={endpoints}
                ingressProtocol={editor.draft.ingressProtocol}
                pending={pending}
                canRemove={editor.draft.targets.length > 1}
                onUpdate={(field, value) => editor.updateTarget(target.clientId, field, value)}
                onRemove={() => editor.removeTarget(target.clientId)}
              />
            ))}
          </div>
        </section>

        {error ? (
          <p className="text-[13px] leading-5 text-danger" role="alert">
            {getModelRouteErrorMessage(error)}
          </p>
        ) : null}

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button disabled={pending} onClick={onClose}>取消</Button>
          <Button type="submit" variant="primary" disabled={pending || sourceConflict !== null}>
            <Save size={15} />
            {pending ? "正在保存" : "保存"}
          </Button>
        </div>
      </form>
    </Surface>
  );
}

function Field({
  label,
  error,
  htmlFor,
  children,
}: {
  label: string;
  error?: string;
  htmlFor: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label htmlFor={htmlFor} className="text-sm font-medium">{label}</label>
      <div className="mt-2">{children}</div>
      {error ? (
        <p id={`${htmlFor}-error`} className="mt-1.5 text-xs text-danger">{error}</p>
      ) : null}
    </div>
  );
}

function readProtocolDialect(value: string): ProtocolDialect {
  return value === "anthropic_messages" ? "anthropic_messages" : "openai_responses";
}

const inputClass =
  "focus-ring h-8 w-full rounded-control border border-subtle bg-surface px-2.5 text-[12px] text-primary placeholder:text-tertiary disabled:opacity-60";
