import { RotateCw, Save } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";

import type {
  ProviderCredential,
  ProviderCredentialCreateInput,
  ProviderCredentialRotateInput,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import type { ProxyConfiguration, ProxyProfile } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { controlClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

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
  const rotating = mode === "rotate";
  const editing = mode === "edit";
  const primaryRef = useRef<HTMLInputElement>(null);
  const [label, setLabel] = useState(credential?.label ?? "");
  const [proxyId, setProxyId] = useState(
    credential?.proxyProfileId ?? directProxyId(proxies),
  );
  const [maxConcurrency, setMaxConcurrency] = useState(
    String(credential?.maxConcurrency ?? 4),
  );
  const [enabled, setEnabled] = useState(credential?.enabled ?? true);
  const [apiKey, setApiKey] = useState("");
  const [errors, setErrors] = useState<Record<string, string>>({});

  const options = useMemo(
    () =>
      [...proxies.items].sort((left, right) => {
        if (left.kind === "direct") return -1;
        if (right.kind === "direct") return 1;
        return left.name.localeCompare(right.name, "zh-CN");
      }),
    [proxies.items],
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

    try {
      if (mode === "create") {
        await onSubmit({
          mode: "create",
          input: {
            expectedRevision: configRevision,
            label,
            proxyProfileId: proxyId,
            maxConcurrency: Number(maxConcurrency),
            enabled,
            apiKey,
          },
        });
      } else if (mode === "edit" && credential) {
        await onSubmit({
          mode: "edit",
          id: credential.id,
          input: {
            expectedRevision: configRevision,
            expectedConfigVersion: credential.configVersion,
            label,
            proxyProfileId: proxyId,
            maxConcurrency: Number(maxConcurrency),
            enabled,
          },
        });
      } else if (mode === "rotate" && credential) {
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
    } catch {
      // Parent keeps the draft mounted.
    }
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void submit(event)} noValidate>
      {sourceConflict ? (
        <p className="rounded-[8px] bg-surface-muted px-3 py-2 text-[13px] text-warning" role="status">
          {sourceConflict === "deleted"
            ? "此 API Key 已从最新配置中删除；草稿仍保留，请复制需要的内容后关闭。"
            : "此 API Key 已被其他操作修改；草稿仍保留，请关闭后重新打开并审阅最新值。"}
        </p>
      ) : null}

      {!rotating ? (
        <>
          <Field label="名称" error={errors.label} htmlFor="credential-label">
            <input
              id="credential-label"
              ref={editing ? primaryRef : undefined}
              className={controlClass(Boolean(errors.label))}
              value={label}
              maxLength={100}
              autoComplete="off"
              disabled={pending || sourceConflict !== null}
              onChange={(event) => setLabel(event.target.value)}
            />
          </Field>
          <Field label="代理" htmlFor="credential-proxy">
            <select
              id="credential-proxy"
              className={controlClass()}
              value={proxyId}
              disabled={pending || sourceConflict !== null}
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
              className={controlClass(Boolean(errors.maxConcurrency))}
              type="number"
              min={1}
              max={10_000}
              step={1}
              value={maxConcurrency}
              disabled={pending || sourceConflict !== null}
              onChange={(event) => setMaxConcurrency(event.target.value)}
            />
          </Field>
        </>
      ) : null}

      {!editing ? (
        <Field
          label={rotating ? "新 API Key" : "API Key"}
          error={errors.apiKey}
          htmlFor="credential-secret"
        >
          <input
            id="credential-secret"
            ref={rotating || mode === "create" ? primaryRef : undefined}
            className={controlClass(Boolean(errors.apiKey))}
            type="password"
            value={apiKey}
            disabled={pending || sourceConflict !== null}
            autoComplete="new-password"
            spellCheck={false}
            onChange={(event) => setApiKey(event.target.value)}
          />
        </Field>
      ) : null}

      {!rotating ? (
        <div className="flex items-center justify-between gap-4">
          <p id="credential-enabled-label" className="text-[13px] font-medium">
            启用此 API Key
          </p>
          <Switch
            id="credential-enabled"
            checked={enabled}
            disabled={pending || sourceConflict !== null}
            aria-labelledby="credential-enabled-label"
            onCheckedChange={setEnabled}
          />
        </div>
      ) : null}

      <FormError>{error ? getProviderErrorMessage(error) : null}</FormError>

      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending || sourceConflict !== null}>
          {rotating ? <RotateCw size={14} /> : <Save size={14} />}
          {pending ? "正在保存" : rotating ? "轮换" : "保存"}
        </Button>
      </div>
    </form>
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
  return (
    value.length > 0 &&
    value.length <= 8192 &&
    [...value].every((character) => {
      const code = character.charCodeAt(0);
      return code >= 0x21 && code <= 0x7e;
    })
  );
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

