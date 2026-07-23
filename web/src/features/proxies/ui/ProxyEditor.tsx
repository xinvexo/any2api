import { useEffect, useRef, type FormEvent } from "react";

import type { ProxyProfile } from "../api/proxy-contracts";
import { getProxyErrorMessage } from "../model/proxy-error";
import {
  useProxyEditor,
  type ProxyEditorSubmit,
} from "../model/use-proxy-editor";
import { Button } from "@/shared/ui/Button";
import { controlClass, selectClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

interface ProxyEditorProps {
  profile?: ProxyProfile;
  isGlobal: boolean;
  configRevision: number;
  pending: boolean;
  error: unknown;
  onSubmit: (submit: ProxyEditorSubmit) => Promise<void>;
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
    const next = editor.buildSubmit(configRevision);
    if (!next) {
      focusInvalidAfterRender.current = true;
      return;
    }
    try {
      await onSubmit(next);
    } catch {
      // Mutation state renders the structured server error without discarding the draft.
    }
  }

  return (
    <form ref={formRef} className="space-y-5" onSubmit={(event) => void submit(event)} noValidate>
      <Field label="名称" error={editor.errors.name} htmlFor="proxy-name">
        <input
          id="proxy-name"
          ref={nameRef}
          className={controlClass(Boolean(editor.errors.name))}
          value={editor.draft.name}
          maxLength={100}
          autoComplete="off"
          aria-invalid={Boolean(editor.errors.name)}
          aria-describedby={editor.errors.name ? "proxy-name-error" : undefined}
          onChange={(event) => editor.update("name", event.target.value)}
        />
      </Field>

      <Field label="类型" htmlFor="proxy-kind">
        <select
          id="proxy-kind"
          className={selectClass()}
          value={editor.draft.kind}
          onChange={(event) =>
            editor.update("kind", event.target.value === "socks5" ? "socks5" : "http")
          }
        >
          <option value="http">HTTP</option>
          <option value="socks5">SOCKS5</option>
        </select>
      </Field>

      <Field label="主机" error={editor.errors.host} htmlFor="proxy-host">
        <input
          id="proxy-host"
          className={controlClass(Boolean(editor.errors.host))}
          value={editor.draft.host}
          placeholder="proxy.example.com"
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
          className={controlClass(Boolean(editor.errors.port))}
          value={editor.draft.port}
          inputMode="numeric"
          placeholder="8080"
          aria-invalid={Boolean(editor.errors.port)}
          aria-describedby={editor.errors.port ? "proxy-port-error" : undefined}
          onChange={(event) => editor.update("port", event.target.value)}
        />
      </Field>

      <div className="space-y-4">
        <div className="flex items-center justify-between gap-4">
          <p id="proxy-auth-label" className="text-[13px] font-medium">
            出口代理认证
          </p>
          <Switch
            id="proxy-auth-enabled"
            checked={editor.draft.authEnabled}
            aria-labelledby="proxy-auth-label"
            onCheckedChange={(checked) => {
              editor.update("authEnabled", checked);
              if (!checked) {
                editor.update("username", "");
                editor.update("password", "");
              } else if (profile?.passwordConfigured) {
                editor.update("username", profile.username ?? "");
              }
            }}
          />
        </div>

        {editor.draft.authEnabled ? (
          <>
            <Field label="用户名" error={editor.errors.username} htmlFor="proxy-auth-username">
              <input
                id="proxy-auth-username"
                className={controlClass(Boolean(editor.errors.username))}
                value={editor.draft.username}
                autoComplete="username"
                aria-invalid={Boolean(editor.errors.username)}
                aria-describedby={editor.errors.username ? "proxy-auth-username-error" : undefined}
                onChange={(event) => editor.update("username", event.target.value)}
              />
            </Field>
            <Field label="密码" error={editor.errors.password} htmlFor="proxy-auth-password">
              <input
                id="proxy-auth-password"
                type="password"
                className={controlClass(Boolean(editor.errors.password))}
                value={editor.draft.password}
                autoComplete="new-password"
                placeholder={profile?.passwordConfigured ? "留空则保留原密码" : undefined}
                aria-invalid={Boolean(editor.errors.password)}
                aria-describedby={editor.errors.password ? "proxy-auth-password-error" : undefined}
                onChange={(event) => editor.update("password", event.target.value)}
              />
            </Field>
          </>
        ) : null}
      </div>

      <div className="flex items-center justify-between gap-4">
        <p id="proxy-enabled-label" className="text-[13px] font-medium">
          启用此出口代理
        </p>
        <Switch
          id="proxy-enabled"
          checked={editor.draft.enabled}
          disabled={isGlobal}
          aria-labelledby="proxy-enabled-label"
          onCheckedChange={(checked) => editor.update("enabled", checked)}
        />
      </div>

      <FormError>{error ? getProxyErrorMessage(error) : null}</FormError>

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending}>
          保存
        </Button>
      </div>
    </form>
  );
}
