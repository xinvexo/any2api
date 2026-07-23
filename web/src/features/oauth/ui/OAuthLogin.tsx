import {
  CheckCircle2,
  ExternalLink,
  FileDown,
  LoaderCircle,
  LogIn,
  RotateCcw,
} from "lucide-react";
import { useState, type FormEvent } from "react";
import { useSearchParams } from "react-router-dom";

import type { OAuthProvider } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import {
  isOAuthProvider,
  oauthProviderLabel,
  OAUTH_PROVIDER_OPTIONS,
} from "../model/oauth-provider-catalog";
import { useOAuthLogin } from "../model/use-oauth-login";
import { OAuthProviderNav } from "./OAuthProviderNav";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/form-field";
import { controlClass } from "@/shared/ui/form-control";
import { Surface } from "@/shared/ui/Surface";

export function OAuthLogin() {
  const login = useOAuthLogin();
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedProvider = resolveSelectedProvider(searchParams.get("kind"));
  const [callbackUrl, setCallbackUrl] = useState("");
  const callbackReady = callbackUrl.trim().length > 0;
  const providerName = oauthProviderLabel(selectedProvider);
  const activeSession =
    login.session && login.session.provider === selectedProvider ? login.session : null;

  function selectProvider(next: OAuthProvider) {
    if (next === selectedProvider) {
      return;
    }
    setCallbackUrl("");
    login.reset();
    setSearchParams(
      (current) => {
        const params = new URLSearchParams(current);
        params.set("kind", next);
        return params;
      },
      { replace: true },
    );
  }

  async function startLogin() {
    setCallbackUrl("");
    try {
      await login.start(selectedProvider);
    } catch {
      // The hook exposes the safe user-facing error state.
    }
  }

  async function exchange(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!callbackReady) {
      return;
    }
    try {
      await login.exchange(callbackUrl.trim());
      setCallbackUrl("");
    } catch {
      // A failed exchange consumes the one-time session; the next attempt starts fresh.
    }
  }

  function restart() {
    setCallbackUrl("");
    login.reset();
  }

  return (
    /*
     * Desktop grid:
     *   row1 col2 = right-only actions (does not sit above kinds)
     *   row2 col1 = kinds, row2 col2 = panel → first kind top == panel top
     */
    <div
      className="grid grid-cols-1 gap-x-5 gap-y-3 sm:grid-cols-[13rem_minmax(0,1fr)] lg:grid-cols-[14rem_minmax(0,1fr)]"
      aria-busy={login.pending !== null}
    >
      <div className="flex justify-end sm:col-start-2 sm:row-start-1">
        <div className="flex shrink-0 items-center gap-1.5">
          {activeSession ? (
            <>
              <a
                href={activeSession.authorizationUrl}
                target="_blank"
                rel="noreferrer"
                className="focus-ring inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-[7px] bg-surface-muted px-3 text-[13px] font-medium text-primary transition-colors hover:bg-surface-hover"
              >
                <ExternalLink size={14} aria-hidden="true" />
                打开授权页
              </a>
              <Button variant="ghost" disabled={login.pending !== null} onClick={restart}>
                <RotateCcw size={14} aria-hidden="true" />
                重新开始
              </Button>
            </>
          ) : (
            <Button
              variant="primary"
              disabled={login.pending !== null}
              onClick={() => void startLogin()}
            >
              {login.pending === "start" ? (
                <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
              ) : (
                <LogIn size={14} aria-hidden="true" />
              )}
              OAuth认证
            </Button>
          )}
        </div>
      </div>

      <div className="sm:col-start-1 sm:row-start-2">
        <OAuthProviderNav selected={selectedProvider} onSelect={selectProvider} />
      </div>

      <div className="min-w-0 sm:col-start-2 sm:row-start-2">
        {activeSession ? (
          <Surface className="overflow-hidden">
            <div className="space-y-4 px-3 py-3 sm:px-4 sm:py-4">
              <div className="min-w-0">
                <p className="text-[13px] font-semibold tracking-tight text-primary">
                  {providerName} 授权会话
                </p>
                <p className="mt-1 truncate font-mono text-[11px] text-secondary">
                  {activeSession.redirectUri}
                </p>
              </div>

              <form
                className="space-y-4 border-t border-subtle pt-4"
                onSubmit={(event) => void exchange(event)}
              >
                <Field
                  label="回调 URL"
                  htmlFor="oauth-callback-url"
                  hint={`期望跳转：${activeSession.redirectUri}`}
                >
                  <input
                    id="oauth-callback-url"
                    type="url"
                    inputMode="url"
                    autoComplete="off"
                    spellCheck={false}
                    className={controlClass(false, "font-mono")}
                    value={callbackUrl}
                    placeholder={activeSession.redirectUri}
                    disabled={login.pending !== null}
                    onChange={(event) => setCallbackUrl(event.target.value)}
                  />
                </Field>
                <div className="flex justify-end">
                  <Button
                    type="submit"
                    variant="primary"
                    disabled={!callbackReady || login.pending !== null}
                  >
                    {login.pending === "exchange" ? (
                      <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
                    ) : (
                      <FileDown size={14} aria-hidden="true" />
                    )}
                    下载 JSON
                  </Button>
                </div>
              </form>
            </div>
          </Surface>
        ) : (
          <Surface className="flex min-h-48 flex-col items-center justify-center px-4 py-10 text-center">
            <p className="text-[13px] font-medium">还没有 {providerName} 登录会话</p>
            <p className="mt-1 max-w-sm text-[12px] text-secondary">
              点击「OAuth认证」生成一次性授权链接，完成后粘贴回调 URL 下载认证 JSON。
            </p>
          </Surface>
        )}

        {login.error ? (
          <p className="pt-2 text-sm text-danger" role="alert">
            {getOAuthErrorMessage(login.error)}
          </p>
        ) : null}
        {login.completedFilename ? (
          <p className="flex items-center gap-2 pt-2 text-[12px] text-success" role="status">
            <CheckCircle2 size={14} aria-hidden="true" />
            已下载 {login.completedFilename}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function resolveSelectedProvider(value: string | null): OAuthProvider {
  if (isOAuthProvider(value)) {
    return value;
  }
  return OAUTH_PROVIDER_OPTIONS[0]?.provider ?? "codex";
}
