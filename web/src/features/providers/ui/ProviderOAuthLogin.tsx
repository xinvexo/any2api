import { ExternalLink, LogIn, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";

import type { ProviderEndpoint } from "../api/provider-contracts";
import type { ProviderOAuthExchangeResult } from "../api/provider-credential-contracts";
import { getProviderErrorMessage } from "../model/provider-error";
import { useProviderOAuth } from "../model/use-provider-oauth";
import type { ProxyConfiguration } from "@/features/proxies";
import { Button } from "@/shared/ui/Button";
import { controlClass, selectClass } from "@/shared/ui/form-control";
import { Field, FormError } from "@/shared/ui/form-field";
import { Switch } from "@/shared/ui/Switch";

export function ProviderOAuthLogin({
  endpoint,
  configRevision,
  proxies,
  onComplete,
  onClose,
}: {
  endpoint: ProviderEndpoint;
  configRevision: number;
  proxies: ProxyConfiguration;
  onComplete: (result: ProviderOAuthExchangeResult) => Promise<void>;
  onClose: () => void;
}) {
  const oauth = useProviderOAuth(endpoint.id);
  const primaryRef = useRef<HTMLInputElement>(null);
  const callbackRef = useRef<HTMLTextAreaElement>(null);
  const [label, setLabel] = useState(`${endpoint.name} OAuth`);
  const [proxyId, setProxyId] = useState(directProxyId(proxies));
  const [maxConcurrency, setMaxConcurrency] = useState("4");
  const [enabled, setEnabled] = useState(true);
  const [callbackUrl, setCallbackUrl] = useState("");
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

  async function begin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextErrors = validateStart(label, maxConcurrency);
    setErrors(nextErrors);
    if (Object.keys(nextErrors).length > 0) return;
    try {
      await oauth.start({
        expectedRevision: configRevision,
        label,
        proxyProfileId: proxyId,
        maxConcurrency: Number(maxConcurrency),
        enabled,
      });
    } catch {
      // Keep the form mounted for retry.
    }
  }

  async function finish(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const value = callbackUrl.trim();
    if (!value) {
      setErrors({ callbackUrl: "请粘贴浏览器地址栏中的完整回调 URL" });
      return;
    }
    setErrors({});
    try {
      const result = await oauth.exchange(value);
      setCallbackUrl("");
      await onComplete(result);
    } catch {
      // The server consumes each callback once; the form returns to a clean retry state.
    }
  }

  if (oauth.session) {
    return (
      <form className="space-y-5" onSubmit={(event) => void finish(event)} noValidate>
        <div className="rounded-[10px] border border-subtle bg-surface-muted px-4 py-3 text-[13px] leading-5 text-secondary">
          <p className="font-medium text-primary">在新窗口完成 {providerLabel(endpoint)} 授权</p>
          <p className="mt-1">
            授权后 localhost 页面可能显示无法访问，这是正常的。复制浏览器地址栏中的完整 URL，粘贴到下方。
          </p>
          <a
            className="focus-ring mt-3 inline-flex items-center gap-1.5 rounded-[7px] text-accent hover:underline"
            href={oauth.session.authorizationUrl}
            target="_blank"
            rel="noreferrer"
          >
            重新打开授权页面
            <ExternalLink size={13} />
          </a>
        </div>

        <Field label="回调 URL" error={errors.callbackUrl} htmlFor="provider-oauth-callback">
          <textarea
            id="provider-oauth-callback"
            ref={callbackRef}
            className={`${controlClass(Boolean(errors.callbackUrl))} min-h-28 resize-y py-2 font-mono text-[12px]`}
            value={callbackUrl}
            autoComplete="off"
            spellCheck={false}
            disabled={oauth.pending}
            placeholder={oauth.session.redirectUri}
            onChange={(event) => setCallbackUrl(event.target.value)}
          />
        </Field>

        <p className="text-[12px] text-tertiary">
          本次登录会话约 {Math.round(oauth.session.expiresInSeconds / 60)} 分钟后失效；回调只能使用一次。
        </p>
        <FormError>{oauth.error ? getProviderErrorMessage(oauth.error) : null}</FormError>

        <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
          <Button type="button" variant="ghost" disabled={oauth.pending} onClick={onClose}>
            取消
          </Button>
          <Button
            type="button"
            variant="ghost"
            disabled={oauth.pending}
            onClick={() => {
              oauth.reset();
              setCallbackUrl("");
            }}
          >
            <RefreshCw size={14} />
            重新开始
          </Button>
          <Button type="submit" variant="primary" disabled={oauth.pending}>
            <LogIn size={14} />
            {oauth.pending ? "正在交换 Token" : "完成登录并拉取模型"}
          </Button>
        </div>
      </form>
    );
  }

  return (
    <form className="space-y-5" onSubmit={(event) => void begin(event)} noValidate>
      <div className="rounded-[10px] bg-surface-muted px-4 py-3 text-[13px] leading-5 text-secondary">
        <p>any2api 会使用所选代理访问 OAuth Token Endpoint，并将 Token 加密保存；管理页面不会显示 Token。</p>
        {endpoint.providerKind === "codex" ? (
          <p className="mt-2 text-warning">
            Codex OAuth 数据面通常需要 Base URL 为 https://chatgpt.com/backend-api/codex。实际请求始终使用你填写的 Base URL，不会被自动改写。
          </p>
        ) : null}
      </div>

      <Field label="名称" error={errors.label} htmlFor="provider-oauth-label">
        <input
          id="provider-oauth-label"
          ref={primaryRef}
          className={controlClass(Boolean(errors.label))}
          value={label}
          maxLength={100}
          autoComplete="off"
          disabled={oauth.pending}
          onChange={(event) => setLabel(event.target.value)}
        />
      </Field>
      <Field label="代理" htmlFor="provider-oauth-proxy">
        <select
          id="provider-oauth-proxy"
          className={selectClass()}
          value={proxyId}
          disabled={oauth.pending}
          onChange={(event) => setProxyId(event.target.value)}
        >
          {options.map((proxy) => (
            <option key={proxy.id} value={proxy.id}>
              {proxy.name}
            </option>
          ))}
        </select>
      </Field>
      <Field label="最大并发" error={errors.maxConcurrency} htmlFor="provider-oauth-concurrency">
        <input
          id="provider-oauth-concurrency"
          className={controlClass(Boolean(errors.maxConcurrency))}
          type="number"
          min={1}
          max={10_000}
          step={1}
          value={maxConcurrency}
          disabled={oauth.pending}
          onChange={(event) => setMaxConcurrency(event.target.value)}
        />
      </Field>
      <div className="flex items-center justify-between gap-4">
        <p id="provider-oauth-enabled-label" className="text-[13px] font-medium">
          登录后立即启用
        </p>
        <Switch
          id="provider-oauth-enabled"
          checked={enabled}
          disabled={oauth.pending}
          aria-labelledby="provider-oauth-enabled-label"
          onCheckedChange={setEnabled}
        />
      </div>

      <FormError>{oauth.error ? getProviderErrorMessage(oauth.error) : null}</FormError>
      <div className="flex flex-col-reverse gap-2 border-t border-subtle pt-4 sm:flex-row sm:justify-end">
        <Button type="button" variant="ghost" disabled={oauth.pending} onClick={onClose}>
          取消
        </Button>
        <Button type="submit" variant="primary" disabled={oauth.pending}>
          <LogIn size={14} />
          {oauth.pending ? "正在创建登录会话" : `使用 ${providerLabel(endpoint)} 登录`}
        </Button>
      </div>
    </form>
  );
}

function validateStart(label: string, concurrency: string) {
  const errors: Record<string, string> = {};
  if (label.trim() !== label || label.length === 0) {
    errors.label = "名称不能为空，且首尾不能包含空格";
  }
  const numeric = Number(concurrency);
  if (!Number.isInteger(numeric) || numeric < 1 || numeric > 10_000) {
    errors.maxConcurrency = "最大并发必须是 1 到 10000 的整数";
  }
  return errors;
}

function directProxyId(configuration: ProxyConfiguration) {
  return configuration.items.find((proxy) => proxy.kind === "direct")?.id ?? "";
}

function providerLabel(endpoint: ProviderEndpoint) {
  return endpoint.providerKind === "codex" ? "OpenAI / Codex" : "Anthropic / Claude";
}
