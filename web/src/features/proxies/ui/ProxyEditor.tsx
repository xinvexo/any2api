import { Save } from "lucide-react";
import { useEffect, useRef, type FormEvent } from "react";

import type { ProxyProfile, ProxyWriteInput } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import { useProxyEditor } from "../model/use-proxy-editor";
import { Button } from "@/shared/ui/Button";

interface ProxyEditorProps {
  profile?: ProxyProfile;
  isGlobal: boolean;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (input: ProxyWriteInput) => Promise<void>;
  onClose: () => void;
}

export function ProxyEditor({
  profile,
  isGlobal,
  configRevision,
  pending,
  error,
  onSubmit,
  onClose,
}: ProxyEditorProps) {
  const editor = useProxyEditor(profile);
  const hasValidationErrors = Object.keys(editor.errors).length > 0;
  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    nameRef.current?.focus();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const input = editor.buildInput(configRevision);
    if (!input) {
      return;
    }
    try {
      await onSubmit(input);
      onClose();
    } catch {
      // Mutation state renders the structured server error without discarding the draft.
    }
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)}>
      {hasValidationErrors ? (
        <p className="rounded-[10px] bg-surface-muted px-3 py-2 text-sm text-danger" role="alert">
          请检查表单中标记的字段。
        </p>
      ) : null}

      <Field label="名称" error={editor.errors.name} htmlFor="proxy-name">
        <input
          id="proxy-name"
          ref={nameRef}
          className={inputClass}
          value={editor.draft.name}
          maxLength={100}
          required
          autoComplete="off"
          aria-invalid={Boolean(editor.errors.name)}
          aria-describedby={editor.errors.name ? "proxy-name-error" : undefined}
          onChange={(event) => editor.update("name", event.target.value)}
        />
      </Field>

      <Field label="类型" htmlFor="proxy-kind">
        <select
          id="proxy-kind"
          className={inputClass}
          value={editor.draft.kind}
          onChange={(event) =>
            editor.update("kind", event.target.value === "socks5" ? "socks5" : "http")
          }
        >
          <option value="http">HTTP（默认代理端 DNS）</option>
          <option value="socks5">SOCKS5（默认远端 DNS）</option>
        </select>
      </Field>

      <Field label="主机" error={editor.errors.host} htmlFor="proxy-host">
        <input
          id="proxy-host"
          className={inputClass}
          value={editor.draft.host}
          placeholder="proxy.example.com"
          required
          autoComplete="off"
          spellCheck={false}
          aria-invalid={Boolean(editor.errors.host)}
          aria-describedby={editor.errors.host ? "proxy-host-error" : undefined}
          onChange={(event) => editor.update("host", event.target.value)}
        />
      </Field>

      <Field label="端口" error={editor.errors.port} htmlFor="proxy-port">
        <input
          id="proxy-port"
          className={inputClass}
          value={editor.draft.port}
          inputMode="numeric"
          placeholder="8080"
          required
          aria-invalid={Boolean(editor.errors.port)}
          aria-describedby={editor.errors.port ? "proxy-port-error" : undefined}
          onChange={(event) => editor.update("port", event.target.value)}
        />
      </Field>

      <div className="flex items-start gap-3 rounded-[10px] bg-surface-muted px-4 py-3">
        <input
          id="proxy-enabled"
          type="checkbox"
          className="mt-0.5 size-4 accent-accent"
          checked={editor.draft.enabled}
          disabled={isGlobal}
          onChange={(event) => editor.update("enabled", event.target.checked)}
        />
        <div>
          <label htmlFor="proxy-enabled" className="text-sm font-medium">
            启用此代理
          </label>
          <p className="mt-1 text-xs leading-5 text-secondary">
            {isGlobal
              ? "当前为全局代理；请先切换全局出口，再停用此代理。"
              : "停用后不能设为全局，也不会成为 Credential 的可用出口。"}
          </p>
        </div>
      </div>

      {error ? (
        <p className="text-sm text-danger" role="alert">
          {getProxyErrorMessage(error)}
        </p>
      ) : null}

      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending}>
          <Save size={15} />
          {pending ? "正在保存" : "保存"}
        </Button>
      </div>
    </form>
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
  children: React.ReactNode;
}) {
  return (
    <div>
      <label htmlFor={htmlFor} className="text-sm font-medium">
        {label}
      </label>
      <div className="mt-2">{children}</div>
      {error ? (
        <p id={`${htmlFor}-error`} className="mt-1.5 text-xs text-danger" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

const inputClass =
  "focus-ring h-10 w-full rounded-[10px] border-0 bg-surface-muted px-3 text-sm text-primary disabled:opacity-60";
