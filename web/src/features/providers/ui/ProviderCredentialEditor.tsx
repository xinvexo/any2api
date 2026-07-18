import { KeyRound, RotateCw, Save, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type FormEvent, type ReactNode } from "react";

import type {
  ProviderCredential,
  ProviderCredentialCreateInput,
  ProviderCredentialRotateInput,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import type { ProxyConfiguration, ProxyProfile } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { Surface } from "@/shared/ui/Surface";

export type CredentialEditorSubmission =
  | { mode: "create"; input: ProviderCredentialCreateInput }
  | { mode: "edit"; id: string; input: ProviderCredentialUpdateInput }
  | { mode: "rotate"; id: string; label: string; input: ProviderCredentialRotateInput };

interface ProviderCredentialEditorProps {
  mode: "create" | "edit" | "rotate";
  credential?: ProviderCredential;
  sourceConflict: "changed" | "deleted" | null;
  configRevision: number;
  proxies: ProxyConfiguration;
  pending: boolean;
  error: unknown;
  onSubmit: (submission: CredentialEditorSubmission) => Promise<void>;
  onClose: () => void;
}

export function ProviderCredentialEditor({
  mode,
  credential,
  sourceConflict,
  configRevision,
  proxies,
  pending,
  error,
  onSubmit,
  onClose,
}: ProviderCredentialEditorProps) {
  const [label, setLabel] = useState(credential?.label ?? "");
  const [proxyId, setProxyId] = useState(credential?.proxyProfileId ?? directProxyId(proxies));
  const [maxConcurrency, setMaxConcurrency] = useState(
    String(credential?.maxConcurrency ?? 1),
  );
  const [enabled, setEnabled] = useState(credential?.enabled ?? true);
  const [apiKey, setApiKey] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});
  const primaryRef = useRef<HTMLInputElement>(null);
  const rotating = mode === "rotate";
  const editing = mode === "edit";
  const options = useMemo(
    () => proxies.items.filter((proxy) => proxy.enabled || proxy.id === proxyId),
    [proxies.items, proxyId],
  );

  useEffect(() => {
    primaryRef.current?.focus();
  }, []);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sourceConflict) {
      return;
    }
    const nextErrors = validate(mode, label, maxConcurrency, apiKey);
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) {
      return;
    }
    const concurrency = Number(maxConcurrency);
    if (mode === "create") {
      await onSubmit({
        mode,
        input: {
          expectedRevision: configRevision,
          label,
          apiKey,
          proxyProfileId: proxyId,
          maxConcurrency: concurrency,
          enabled,
        },
      });
    } else if (mode === "edit" && credential) {
      await onSubmit({
        mode,
        id: credential.id,
        input: {
          expectedRevision: configRevision,
          expectedConfigVersion: credential.configVersion,
          label,
          proxyProfileId: proxyId,
          maxConcurrency: concurrency,
          enabled,
        },
      });
    } else if (credential) {
      await onSubmit({
        mode: "rotate",
        id: credential.id,
        label: credential.label,
        input: {
          expectedRevision: configRevision,
          expectedConfigVersion: credential.configVersion,
          expectedSecretVersion: credential.secretVersion,
          apiKey,
        },
      });
    }
  }

  return (
    <Surface className="h-fit overflow-hidden lg:sticky lg:top-24">
      <div className="flex items-start justify-between border-b border-subtle px-5 py-4 sm:px-6">
        <div className="flex min-w-0 gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-control bg-surface-muted text-accent-copy">
            {rotating ? <RotateCw size={17} /> : <KeyRound size={17} />}
          </span>
          <div className="min-w-0">
            <h2 className="break-words font-semibold [overflow-wrap:anywhere]">
              {mode === "create" ? "新增 API Key" : rotating ? `轮换 ${credential?.label ?? "API Key"}` : "编辑 API Key"}
            </h2>
            <p className="mt-1 text-sm text-secondary">
              {rotating ? "替换上游认证材料" : "配置代理与最大并发"}
            </p>
          </div>
        </div>
        <Button
          variant="ghost"
          className="size-9 shrink-0 px-0"
          onClick={onClose}
          disabled={pending}
          aria-label="关闭编辑器"
        >
          <X size={17} />
        </Button>
      </div>

      <form className="space-y-5 p-5 sm:p-6" onSubmit={(event) => void submit(event)}>
        {sourceConflict ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-warning" role="status">
            {sourceConflict === "deleted"
              ? "此 API Key 已从最新配置中删除。"
              : "此 API Key 已被其他操作修改，请关闭后重新打开。"}
          </p>
        ) : null}

        {!rotating ? (
          <>
            <Field label="名称" error={errors.label} htmlFor="credential-label">
              <input
                id="credential-label"
                ref={primaryRef}
                className={inputClass}
                value={label}
                maxLength={100}
                required
                disabled={pending}
                autoComplete="off"
                onChange={(event) => setLabel(event.target.value)}
              />
            </Field>
            <Field label="代理" htmlFor="credential-proxy">
              <select
                id="credential-proxy"
                className={inputClass}
                value={proxyId}
                disabled={pending}
                onChange={(event) => setProxyId(event.target.value)}
              >
                {options.map((proxy) => (
                  <option key={proxy.id} value={proxy.id}>
                    {proxyOptionLabel(proxy, proxies)}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="最大并发" error={errors.maxConcurrency} htmlFor="credential-concurrency">
              <input
                id="credential-concurrency"
                className={inputClass}
                type="number"
                min={1}
                max={10_000}
                step={1}
                value={maxConcurrency}
                disabled={pending}
                onChange={(event) => setMaxConcurrency(event.target.value)}
              />
            </Field>
            <div className="flex items-start gap-3 rounded-control border border-subtle bg-surface-muted px-4 py-3">
              <input
                id="credential-enabled"
                type="checkbox"
                className="mt-0.5 size-4 accent-accent"
                checked={enabled}
                disabled={pending}
                onChange={(event) => setEnabled(event.target.checked)}
              />
              <label htmlFor="credential-enabled" className="text-sm font-medium">
                启用此 API Key
              </label>
            </div>
          </>
        ) : null}

        {!editing ? (
          <Field label={rotating ? "新 API Key" : "API Key"} error={errors.apiKey} htmlFor="credential-secret">
            <input
              id="credential-secret"
              ref={rotating ? primaryRef : undefined}
              className={inputClass}
              type="password"
              value={apiKey}
              required
              disabled={pending}
              autoComplete="new-password"
              spellCheck={false}
              onChange={(event) => setApiKey(event.target.value)}
            />
          </Field>
        ) : null}

        {error ? (
          <p className="rounded-control bg-surface-muted px-3 py-2 text-sm text-danger" role="alert">
            {getProviderErrorMessage(error)}
          </p>
        ) : null}

        <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button disabled={pending} onClick={onClose}>取消</Button>
          <Button type="submit" variant="primary" disabled={pending || sourceConflict !== null}>
            {rotating ? <RotateCw size={15} /> : <Save size={15} />}
            {pending ? "正在保存" : rotating ? "轮换" : "保存"}
          </Button>
        </div>
      </form>
    </Surface>
  );
}

function validate(mode: string, label: string, concurrency: string, apiKey: string) {
  const errors: Record<string, string> = {};
  if (mode !== "rotate" && (label.trim() !== label || label.length === 0)) {
    errors.label = "名称不能为空，且首尾不能包含空格";
  }
  const numeric = Number(concurrency);
  if (mode !== "rotate" && (!Number.isInteger(numeric) || numeric < 1 || numeric > 10_000)) {
    errors.maxConcurrency = "最大并发必须是 1 到 10000 的整数";
  }
  if (mode !== "edit" && !validApiKey(apiKey)) {
    errors.apiKey = "API Key 必须为 1 到 8192 个可见 ASCII 字符";
  }
  return errors;
}

function validApiKey(value: string) {
  return value.length > 0 && value.length <= 8192 && [...value].every((character) => {
    const code = character.charCodeAt(0);
    return code >= 0x21 && code <= 0x7e;
  });
}

function directProxyId(configuration: ProxyConfiguration) {
  return configuration.items.find((proxy) => proxy.kind === "direct")?.id ?? "";
}

function proxyOptionLabel(proxy: ProxyProfile, configuration: ProxyConfiguration) {
  if (proxy.kind !== "direct") {
    return `${proxy.name} · ${proxy.kind.toUpperCase()}${proxy.enabled ? "" : " · 已停用"}`;
  }
  const global = configuration.items.find((item) => item.id === configuration.globalProxyId);
  return global?.kind === "direct"
    ? "DIRECT（本机直连）"
    : `DIRECT（继承全局：${global?.name ?? "未知代理"}）`;
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
      {error ? <p className="mt-1.5 text-xs text-danger">{error}</p> : null}
    </div>
  );
}

const inputClass =
  "focus-ring h-10 w-full rounded-control border border-subtle bg-surface px-3 text-sm text-primary placeholder:text-tertiary disabled:opacity-60";
