import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";

import type {
  ProviderCredential,
  ProviderCredentialCreateInput,
  ProviderCredentialUpdateInput,
} from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { controlClass, selectClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

export type CredentialEditorSubmission =
  | { mode: "create"; input: ProviderCredentialCreateInput }
  | {
      mode: "edit";
      id: string;
      input: ProviderCredentialUpdateInput;
      /** When set, rotate secret after metadata update. */
      apiKey?: string;
    };

interface ProviderCredentialEditorProps {
  mode: "create" | "edit";
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
        const trimmedKey = apiKey.trim();
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
          apiKey: trimmedKey.length > 0 ? trimmedKey : undefined,
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

      <Field label="名称" error={errors.label} htmlFor="credential-label">
        <input
          id="credential-label"
          ref={primaryRef}
          className={controlClass(Boolean(errors.label))}
          value={label}
          maxLength={100}
          autoComplete="off"
          disabled={pending || sourceConflict !== null}
          onChange={(event) => setLabel(event.target.value)}
        />
      </Field>
      <Field label="出口代理" htmlFor="credential-proxy">
        <select
          id="credential-proxy"
          className={selectClass()}
          value={proxyId}
          disabled={pending || sourceConflict !== null}
          onChange={(event) => setProxyId(event.target.value)}
        >
          {options.map((proxy) => (
            <option key={proxy.id} value={proxy.id}>
              {proxy.name}
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

      <Field
        label="API Key"
        error={errors.apiKey}
        htmlFor="credential-secret"
      >
        <input
          id="credential-secret"
          className={controlClass(Boolean(errors.apiKey))}
          type="password"
          value={apiKey}
          disabled={pending || sourceConflict !== null}
          autoComplete="new-password"
          spellCheck={false}
          placeholder={editing ? "留空则不修改" : undefined}
          onChange={(event) => setApiKey(event.target.value)}
        />
      </Field>

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

      <FormError>{error ? getProviderErrorMessage(error) : null}</FormError>

      <div className="flex items-center justify-end gap-2 border-t border-subtle pt-4">
        <Button type="button" variant="secondary" className="min-w-[4.5rem]" disabled={pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={pending || sourceConflict !== null}>
          保存
        </Button>
      </div>
    </form>
  );
}

function validate(mode: "create" | "edit", label: string, concurrency: string, apiKey: string) {
  const errors: Record<string, string> = {};
  if (label.trim() !== label || label.length === 0) {
    errors.label = "名称不能为空，且首尾不能包含空格";
  }
  const numeric = Number(concurrency);
  if (!Number.isInteger(numeric) || numeric < 1 || numeric > 10_000) {
    errors.maxConcurrency = "最大并发必须是 1 到 10000 的整数";
  }
  if (mode === "create") {
    if (!validApiKey(apiKey)) {
      errors.apiKey = "API Key 必须为 1 到 8192 个可见 ASCII 字符";
    }
  } else if (apiKey.trim().length > 0 && !validApiKey(apiKey.trim())) {
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
