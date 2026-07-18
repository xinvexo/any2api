import { Globe2, Save, X } from "lucide-react";
import { useEffect, useRef, type FormEvent, type ReactNode } from "react";

import type { ProviderEndpoint, ProviderEndpointWriteInput } from "../api/provider-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEditor } from "../model/use-provider-editor";
import { ProviderSecurityOptions } from "./ProviderSecurityOptions";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

interface ProviderEndpointEditorProps {
  endpoint?: ProviderEndpoint;
  editing: boolean;
  sourceConflict: "changed" | "deleted" | null;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProviderEndpointWriteInput) => Promise<void>;
  onClose: () => void;
}

export function ProviderEndpointEditor({
  endpoint,
  editing,
  sourceConflict,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: ProviderEndpointEditorProps) {
  const editor = useProviderEditor(endpoint);
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
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
      // Keep the draft visible after a revision or server validation error.
    }
  }

  return (
    <Surface className="h-fit overflow-hidden lg:sticky lg:top-24">
      <div className="flex items-start justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div className="flex gap-3">
          <span className="grid size-9 place-items-center rounded-control bg-surface-muted text-accent-copy">
            <Globe2 size={17} aria-hidden="true" />
          </span>
          <div>
            <h2 className="font-semibold">{editing ? "编辑 Provider Endpoint" : "新增 Provider Endpoint"}</h2>
            <p className="mt-1 text-sm text-secondary">URL 是固定上游 origin 与可选路径前缀</p>
          </div>
        </div>
        <Button variant="ghost" className="size-9 px-0" onClick={onClose} disabled={pending} aria-label="关闭编辑器">
          <X size={17} />
        </Button>
      </div>

      <form className="space-y-5 p-5 sm:p-6" onSubmit={(event) => void submit(event)}>
        {sourceConflict ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-warning" role="status">
            {sourceConflict === "deleted"
              ? "此 Endpoint 已从最新配置中删除；草稿仍保留，请复制需要的内容后关闭。"
              : "此 Endpoint 已被其他操作修改；草稿仍保留，请关闭后重新打开并审阅最新值。"}
          </p>
        ) : null}
        {Object.keys(editor.errors).length > 0 ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-danger" role="alert">
            请检查表单中标记的字段。
          </p>
        ) : null}

        <Field label="名称" error={editor.errors.name} htmlFor="provider-name">
          <input
            id="provider-name"
            ref={nameRef}
            className={inputClass}
            value={editor.draft.name}
            maxLength={100}
            required
            disabled={pending}
            autoComplete="off"
            aria-invalid={Boolean(editor.errors.name)}
            aria-describedby={editor.errors.name ? "provider-name-error" : undefined}
            onChange={(event) => editor.update("name", event.target.value)}
          />
        </Field>

        <Field label="Provider" htmlFor="provider-kind">
          <select
            id="provider-kind"
            className={inputClass}
            value={editor.draft.providerKind}
            disabled={pending}
            aria-describedby="provider-kind-help"
            onChange={(event) =>
              editor.updateProviderKind(event.target.value === "claude" ? "claude" : "codex")
            }
          >
            <option value="codex">Codex</option>
            <option value="claude">Claude</option>
          </select>
          <p id="provider-kind-help" className="mt-1.5 text-xs text-tertiary">
            首版只支持同协议路由，不会把 Codex 请求转换为 Claude Messages。
          </p>
        </Field>

        <Field label="协议方言" htmlFor="provider-dialect">
          <input
            id="provider-dialect"
            className={inputClass}
            value={editor.draft.providerKind === "codex" ? "OpenAI Responses" : "Anthropic Messages"}
            readOnly
            aria-readonly="true"
            aria-describedby="provider-dialect-help"
          />
          <p id="provider-dialect-help" className="mt-1.5 text-xs text-tertiary">
            方言由首版 Provider 类型固定选择。
          </p>
        </Field>

        <Field label="Base URL" error={editor.errors.baseUrl} htmlFor="provider-base-url">
          <input
            id="provider-base-url"
            className={inputClass}
            value={editor.draft.baseUrl}
            placeholder="https://api.example.com/v1"
            required
            disabled={pending}
            autoComplete="url"
            spellCheck={false}
            aria-invalid={Boolean(editor.errors.baseUrl)}
            aria-describedby={
              editor.errors.baseUrl
                ? "provider-base-url-error provider-base-url-help"
                : "provider-base-url-help"
            }
            onChange={(event) => editor.update("baseUrl", event.target.value)}
          />
          <p id="provider-base-url-help" className="mt-1.5 text-xs leading-5 text-tertiary">
            不要填写 query 或 fragment；固定路径前缀会在协议端点后保留。服务端还会校验公网、私网与 DNS 风险。
          </p>
        </Field>

        <ProviderSecurityOptions
          draft={editor.draft}
          pending={pending}
          onChange={(field, value) => editor.update(field, value)}
        />

        <div className="flex items-start gap-3 rounded-control border border-subtle bg-surface-muted px-4 py-3">
          <input
            id="provider-enabled"
            type="checkbox"
            className="mt-0.5 size-4 accent-accent"
            checked={editor.draft.enabled}
            disabled={pending}
            aria-describedby="provider-enabled-help"
            onChange={(event) => editor.update("enabled", event.target.checked)}
          />
          <span>
            <label htmlFor="provider-enabled" className="block text-sm font-medium">启用此 Endpoint</label>
            <span id="provider-enabled-help" className="mt-1 block text-xs leading-5 text-secondary">
              停用后不会被模型路由选中，但配置仍会保留。
            </span>
          </span>
        </div>

        {error ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-danger" role="alert">
            {getProviderErrorMessage(error)}
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
      {error ? <p id={`${htmlFor}-error`} className="mt-1.5 text-xs text-danger">{error}</p> : null}
    </div>
  );
}

const inputClass =
  "focus-ring h-10 w-full rounded-control border border-subtle bg-surface px-3 text-sm text-primary placeholder:text-tertiary disabled:opacity-60";
