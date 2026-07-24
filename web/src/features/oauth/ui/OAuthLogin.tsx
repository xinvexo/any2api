import {
  CheckCircle2,
  ExternalLink,
  LoaderCircle,
  LogIn,
  RotateCcw,
} from "lucide-react";
import { useState, type FormEvent } from "react";

import type { OAuthProvider, OAuthStartResult } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { oauthProviderLabel } from "../model/oauth-provider-catalog";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/form-field";
import { controlClass } from "@/shared/ui/form-control";
import { SideDrawer } from "@/shared/ui/SideDrawer";

interface OAuthLoginDrawerProps {
  open: boolean;
  provider: OAuthProvider;
  session: OAuthStartResult | null;
  pending: "start" | "exchange" | null;
  error: unknown;
  onClose: () => void;
  onRestart: () => void;
  onExchange: (callbackUrl: string) => Promise<void>;
}

/**
 * Right-side OAuth login flow. The main page no longer keeps an empty
 * session panel; authentication only appears in this drawer.
 */
export function OAuthLoginDrawer({
  open,
  provider,
  session,
  pending,
  error,
  onClose,
  onRestart,
  onExchange,
}: OAuthLoginDrawerProps) {
  const [callbackUrl, setCallbackUrl] = useState("");
  const providerName = oauthProviderLabel(provider);
  const callbackReady = callbackUrl.trim().length > 0;
  const activeSession = session && session.provider === provider ? session : null;

  async function exchange(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!callbackReady) {
      return;
    }
    try {
      await onExchange(callbackUrl.trim());
      setCallbackUrl("");
    } catch {
      // Failed exchange consumes the one-time session; user can restart.
    }
  }

  function close() {
    if (pending !== null) {
      return;
    }
    setCallbackUrl("");
    onClose();
  }

  function restart() {
    setCallbackUrl("");
    onRestart();
  }

  return (
    <SideDrawer
      open={open}
      title={`${providerName} OAuth 认证`}
      description="生成一次性授权链接，完成后粘贴回调 URL 激活服务器账号。"
      onClose={close}
    >
      <div className="space-y-5" aria-busy={pending !== null}>
        {pending === "start" && !activeSession ? (
          <div className="flex min-h-32 flex-col items-center justify-center gap-2 text-sm text-secondary">
            <LoaderCircle size={18} className="animate-spin" aria-hidden="true" />
            正在创建授权会话…
          </div>
        ) : null}

        {activeSession ? (
          <>
            <div className="flex flex-wrap gap-1.5">
              <a
                href={activeSession.authorizationUrl}
                target="_blank"
                rel="noreferrer"
                className="focus-ring inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-[7px] bg-surface-muted px-3 text-[13px] font-medium text-primary transition-colors hover:bg-surface-hover"
              >
                <ExternalLink size={14} aria-hidden="true" />
                打开授权页
              </a>
              <Button variant="ghost" disabled={pending !== null} onClick={restart}>
                <RotateCcw size={14} aria-hidden="true" />
                重新开始
              </Button>
            </div>

            <form className="space-y-4" onSubmit={(event) => void exchange(event)}>
              <Field label="回调 URL" htmlFor="oauth-callback-url">
                <input
                  id="oauth-callback-url"
                  type="url"
                  inputMode="url"
                  autoComplete="off"
                  spellCheck={false}
                  className={controlClass(false, "font-mono")}
                  value={callbackUrl}
                  placeholder="粘贴授权完成后的回调 URL"
                  disabled={pending !== null}
                  onChange={(event) => setCallbackUrl(event.target.value)}
                />
              </Field>
              <div className="flex justify-end">
                <Button type="submit" variant="primary" disabled={!callbackReady || pending !== null}>
                  {pending === "exchange" ? (
                    <LoaderCircle size={14} className="animate-spin" aria-hidden="true" />
                  ) : (
                    <CheckCircle2 size={14} aria-hidden="true" />
                  )}
                  激活账号
                </Button>
              </div>
            </form>
          </>
        ) : null}

        {!activeSession && pending !== "start" ? (
          <div className="space-y-4">
            <p className="text-sm text-secondary">授权会话尚未创建。可重试生成一次性链接。</p>
            <Button variant="primary" disabled={pending !== null} onClick={restart}>
              <LogIn size={14} aria-hidden="true" />
              重新生成
            </Button>
          </div>
        ) : null}

        {error ? (
          <p className="text-sm text-danger" role="alert">
            {getOAuthErrorMessage(error)}
          </p>
        ) : null}
      </div>
    </SideDrawer>
  );
}
