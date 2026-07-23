import {
  CheckCircle2,
  ExternalLink,
  FileDown,
  LoaderCircle,
  LogIn,
} from "lucide-react";
import { useState, type FormEvent } from "react";

import type { OAuthProvider } from "../api/oauth-contracts";
import { getOAuthErrorMessage } from "../model/oauth-error";
import { useOAuthLogin } from "../model/use-oauth-login";
import { Button } from "@/shared/ui/Button";
import { Field } from "@/shared/ui/form-field";
import { controlClass, selectClass } from "@/shared/ui/form-control";
import { Surface } from "@/shared/ui/Surface";

export function OAuthLogin() {
  const login = useOAuthLogin();
  const [provider, setProvider] = useState<OAuthProvider>("codex");
  const [callbackUrl, setCallbackUrl] = useState("");
  const callbackReady = callbackUrl.trim().length > 0;

  async function start(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setCallbackUrl("");
    try {
      await login.start(provider);
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

  function changeProvider(next: OAuthProvider) {
    setProvider(next);
    setCallbackUrl("");
    login.reset();
  }

  return (
    <section className="space-y-5" aria-labelledby="oauth-heading">
      <header>
        <h1 id="oauth-heading" className="text-[17px] font-semibold tracking-tight">
          OAuth2 Login
        </h1>
        <p className="mt-1 text-[12px] leading-5 text-secondary">
          Generate a one-time Codex or Claude authentication JSON file.
        </p>
      </header>

      <Surface className="max-w-3xl p-5 sm:p-6">
        <form className="space-y-5" onSubmit={(event) => void start(event)}>
          <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
            <Field label="Provider" htmlFor="oauth-provider">
              <select
                id="oauth-provider"
                className={selectClass()}
                value={provider}
                disabled={login.pending !== null}
                onChange={(event) => changeProvider(event.target.value as OAuthProvider)}
              >
                <option value="codex">Codex</option>
                <option value="claude">Claude</option>
              </select>
            </Field>
            <Button
              type="submit"
              variant="primary"
              size="lg"
              disabled={login.pending !== null}
            >
              {login.pending === "start" ? (
                <LoaderCircle size={15} className="animate-spin" aria-hidden="true" />
              ) : (
                <LogIn size={15} aria-hidden="true" />
              )}
              Start login
            </Button>
          </div>
        </form>

        {login.session ? (
          <div className="mt-6 border-t border-subtle pt-5">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-primary">
                  {login.session.provider === "codex" ? "Codex" : "Claude"} authorization
                </p>
                <p className="mt-1 truncate font-mono text-[11px] text-tertiary">
                  {login.session.redirectUri}
                </p>
              </div>
              <a
                href={login.session.authorizationUrl}
                target="_blank"
                rel="noreferrer"
                className="focus-ring inline-flex h-8 shrink-0 items-center justify-center gap-1.5 rounded-[7px] bg-surface-muted px-3 text-[13px] font-medium text-primary transition-colors hover:bg-surface-hover"
              >
                <ExternalLink size={14} aria-hidden="true" />
                Open authorization page
              </a>
            </div>

            <form className="mt-5 space-y-4" onSubmit={(event) => void exchange(event)}>
              <Field
                label="Callback URL"
                htmlFor="oauth-callback-url"
                hint={`Expected redirect: ${login.session.redirectUri}`}
              >
                <input
                  id="oauth-callback-url"
                  type="url"
                  inputMode="url"
                  autoComplete="off"
                  spellCheck={false}
                  className={controlClass(false, "font-mono")}
                  value={callbackUrl}
                  placeholder={login.session.redirectUri}
                  disabled={login.pending !== null}
                  onChange={(event) => setCallbackUrl(event.target.value)}
                />
              </Field>
              <div className="flex justify-end">
                <Button
                  type="submit"
                  variant="primary"
                  size="lg"
                  disabled={!callbackReady || login.pending !== null}
                >
                  {login.pending === "exchange" ? (
                    <LoaderCircle size={15} className="animate-spin" aria-hidden="true" />
                  ) : (
                    <FileDown size={15} aria-hidden="true" />
                  )}
                  Download JSON
                </Button>
              </div>
            </form>
          </div>
        ) : null}

        {login.error ? (
          <p className="mt-5 text-[12px] leading-5 text-danger" role="alert">
            {getOAuthErrorMessage(login.error)}
          </p>
        ) : null}
        {login.completedFilename ? (
          <p className="mt-5 flex items-center gap-2 text-[12px] text-success" role="status">
            <CheckCircle2 size={14} aria-hidden="true" />
            Downloaded {login.completedFilename}
          </p>
        ) : null}
      </Surface>
    </section>
  );
}
