import { Save } from "lucide-react";
import { useEffect, useRef, type FormEvent } from "react";

import type {
  ProviderEndpoint,
  ProviderEndpointWriteInput,
  ProviderKind,
} from "../api/provider-contracts";
import { providerKindLabel } from "../model/provider-kind-catalog";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderEditor } from "../model/use-provider-editor";
import { ProviderSecurityOptions } from "./ProviderSecurityOptions";
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
          disabled={pending || sourceConflict !== null}
          aria-invalid={Boolean(editor.errors.name)}
          aria-describedby={editor.errors.name ? "provider-name-error" : undefined}
          onChange={(event) => editor.update("name", event.target.value)}
        />
      </Field>

      <div className="space-y-1.5">
        <p className="text-[12px] font-medium text-secondary">类型</p>
        <p className="text-[13px] text-primary">
          {providerKindLabel(editor.draft.providerKind)}
          <span className="ml-2 text-[12px] text-tertiary">
            {editor.draft.providerKind === "codex" ? "Responses" : "Messages"}
          </span>
        </p>
      </div>

      <Field label="Base URL" error={editor.errors.baseUrl} htmlFor="provider-base-url">
        <input
          id="provider-base-url"
          className={controlClass(Boolean(editor.errors.baseUrl))}
          value={editor.draft.baseUrl}
          placeholder="https://api.example.com/v1"
          autoComplete="url"
          spellCheck={false}
          disabled={pending || sourceConflict !== null}
          aria-invalid={Boolean(editor.errors.baseUrl)}
          aria-describedby={editor.errors.baseUrl ? "provider-base-url-error" : undefined}
          onChange={(event) => editor.update("baseUrl", event.target.value)}
        />
      </Field>

      <ProviderSecurityOptions
        draft={editor.draft}
        pending={pending || sourceConflict !== null}
        onChange={(field, value) => editor.update(field, value)}
      />

      <div className="flex items-center justify-between gap-4">
        <p id="provider-enabled-label" className="text-[13px] font-medium">
          启用此 Endpoint
        </p>
        <Switch
          id="provider-enabled"
          checked={editor.draft.enabled}
          disabled={pending || sourceConflict !== null}
          aria-labelledby="provider-enabled-label"
          onCheckedChange={(checked) => editor.update("enabled", checked)}
        />
      </div>

      <FormError>{error ? getProviderErrorMessage(error) : null}</FormError>

      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending || sourceConflict !== null}>
          <Save size={14} />
          {pending ? "正在保存" : "保存"}
        </Button>
      </div>
    </form>
  );
}
