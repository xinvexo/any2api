import { useState } from "react";

import {
  exchangeOAuthCallback,
  startOAuthLogin,
} from "../api/oauth-api";
import type {
  OAuthProvider,
  OAuthStartResult,
} from "../api/oauth-contracts";

export function useOAuthLogin() {
  const [session, setSession] = useState<OAuthStartResult | null>(null);
  const [pending, setPending] = useState<"start" | "exchange" | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [completedFilename, setCompletedFilename] = useState<string | null>(null);

  async function start(provider: OAuthProvider) {
    setPending("start");
    setError(null);
    setCompletedFilename(null);
    setSession(null);
    try {
      const result = await startOAuthLogin(provider);
      setSession(result);
      return result;
    } catch (nextError) {
      setError(nextError);
      throw nextError;
    } finally {
      setPending(null);
    }
  }

  async function exchange(callbackUrl: string) {
    if (!session) {
      return;
    }
    setPending("exchange");
    setError(null);
    setCompletedFilename(null);
    try {
      const download = await exchangeOAuthCallback(session.sessionId, callbackUrl);
      const filename = download.filename ?? `${session.provider}-auth.json`;
      saveDownload(download.blob, filename);
      setCompletedFilename(filename);
    } catch (nextError) {
      setError(nextError);
      throw nextError;
    } finally {
      setSession(null);
      setPending(null);
    }
  }

  function reset() {
    setSession(null);
    setError(null);
    setCompletedFilename(null);
  }

  return { session, pending, error, completedFilename, start, exchange, reset };
}

function saveDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.hidden = true;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 0);
}
