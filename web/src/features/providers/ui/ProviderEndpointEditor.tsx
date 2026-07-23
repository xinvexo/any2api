import { useEffect, useRef, type FormEvent } from "react";

import type {
  ProviderEndpoint,
  ProviderEndpointWriteInput,
  ProviderKind,
} from "../api/provider-contracts";
import { PROVIDER_KIND_OPTIONS, providerKindLabel } from "../model/provider-kind-catalog";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEditor } from "../model/use-provider-editor";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

interface ProviderEndpointEditorProps {
  endpoint?: ProviderEndpoint;
  defaultKind?: ProviderKind;
  sourceConflict: "changed" | "deleted" | null;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProviderEndpointWriteInput) => Promise<void>;
  onClose: () => void;
}

export function ProviderEndpointEditor({
  endpoint,
  defaultKind = "codex",
  sourceConflict,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: ProviderEndpointEditorProps) {
  const editor = useProviderEditor(endpoint, defaultKind);
  const formRef = useRef<HTMLFormElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const focusInvalidAfterRender = useRef(false);
  const creating = !endpoint;
  const locked = pending || sourceConflict !== null;

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!focusInvalidAfterRender.current) {
      return;
    }
    focusInvalidAfterRender.current = false;
    formRef.current?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
  }, [editor.errors]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sourceConflict) {
      return;
    }
    const input = editor.buildInput(configRevision);
    if (!input) {
      focusInvalidAfterRender.current = true;
      return;
    }
    try {
      await onSubmit(input);
    } catch {
      // Keep the draft visible after a revision or server validation error.
    }
  }

  return (
    <form
      ref={formRef}
      className="space-y-5"
      onSubmit={(event) => void submit(event)}
      noValidate
    >
      {sourceConflict ? (
        <p className="rounded-[8px] bg-surface-muted px-3 py-2 text-[13px] text-warning" role="status">
          {sourceConflict === "deleted"
            ? "此 Endpoint 已从最新配置中删除；草稿仍保留，请复制需要的内容后关闭。"
            : "此 Endpoint 已被其他操作修改；草稿仍保留，请关闭后重新打开并审阅最新值。"}
        </p>
      ) : null}

      <Field label="名称" error={editor.errors.name} htmlFor="provider-name">
        <input
          id="provider-name"
          ref={nameRef}
          className={controlClass(Boolean(editor.errors.name))}
          value={editor.draft.name}
          maxLength={100}
          autoComplete="off"
          disabled={locked}
          aria-invalid={Boolean(editor.errors.name)}
          aria-describedby={editor.errors.name ? "provider-name-error" : undefined}
          onChange={(event) => editor.update("name", event.target.value)}
        />
      </Field>

      {creating ? (
        <Field label="类型" htmlFor="provider-kind">
          <select
            id="provider-kind"
            className={controlClass(false)}
            value={editor.draft.providerKind}
            disabled={locked}
            onChange={(event) =>
              editor.updateProviderKind(event.target.value as ProviderKind)
            }
          >
            {PROVIDER_KIND_OPTIONS.map((option) => (
              <option key={option.kind} value={option.kind}>
                {option.label}
                {option.kind === "codex" ? " · Responses" : " · Messages"}
              </option>
            ))}
          </select>
        </Field>
      ) : (
        <div className="space-y-1.5">
          <p className="text-[12px] font-medium text-secondary">类型</p>
          <p className="text-[13px] text-primary">
            {providerKindLabel(editor.draft.providerKind)}
            <span className="ml-2 text-[12px] text-tertiary">
              {editor.draft.providerKind === "codex" ? "Responses" : "Messages"}
            </span>
          </p>
        </div>
      )}

      <Field label="Base URL" error={editor.errors.baseUrl} htmlFor="provider-base-url">
        <input
          id="provider-base-url"
          className={controlClass(Boolean(editor.errors.baseUrl))}
          value={editor.draft.baseUrl}
          placeholder="https://api.example.com/v1"
          autoComplete="url"
          spellCheck={false}
          disabled={locked}
          aria-invalid={Boolean(editor.errors.baseUrl)}
          aria-describedby={editor.errors.baseUrl ? "provider-base-url-error" : undefined}
          onChange={(event) => editor.update("baseUrl", event.target.value)}
        />
      </Field>

      <div className="flex items-center justify-between gap-4">
        <p id="provider-enabled-label" className="text-[13px] font-medium">
          启用此 Endpoint
        </p>
        <Switch
          id="provider-enabled"
          checked={editor.draft.enabled}
          disabled={locked}
          aria-labelledby="provider-enabled-label"
          onCheckedChange={(checked) => editor.update("enabled", checked)}
        />
      </div>

      <FormError>{error ? getProviderErrorMessage(error) : null}</FormError>

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={locked}>
          保存
        </Button>
      </div>
    </form>
  );
}
