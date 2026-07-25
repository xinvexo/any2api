import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import {
  exchangeOAuthCallback,
  pollOAuthDevice,
  startOAuthLogin,
} from "../api/oauth-api";
import type {
  OAuthActivationResult,
  OAuthProvider,
  OAuthStartResult,
} from "../api/oauth-contracts";
import { oauthQueryKeys } from "./oauth-query-keys";

export function useOAuthLogin() {
  const queryClient = useQueryClient();
  const [session, setSession] = useState<OAuthStartResult | null>(null);
  const [pending, setPending] = useState<"start" | "exchange" | "poll" | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [completedAccount, setCompletedAccount] =
    useState<OAuthActivationResult | null>(null);

  useEffect(() => {
    if (session?.flow !== "device_code") {
      return;
    }
    const controller = new AbortController();
    let timer: number | null = null;

    const schedule = (delaySeconds: number) => {
      timer = window.setTimeout(() => void poll(), delaySeconds * 1_000);
    };
    const poll = async () => {
      setPending("poll");
      try {
        const result = await pollOAuthDevice(session.sessionId, controller.signal);
        if (controller.signal.aborted) {
          return;
        }
        if (result.status === "pending") {
          setPending(null);
          schedule(result.retryAfterSeconds);
          return;
        }
        setCompletedAccount(result.account);
        setPending(null);
        setSession(null);
        void queryClient.invalidateQueries({ queryKey: oauthQueryKeys.accounts });
      } catch (nextError) {
        if (controller.signal.aborted || isAbortError(nextError)) {
          return;
        }
        setError(nextError);
        setPending(null);
        setSession(null);
      }
    };

    // A zero-delay timer survives React StrictMode's setup/cleanup probe without
    // issuing a duplicate one-time poll request.
    schedule(0);
    return () => {
      controller.abort();
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
  }, [queryClient, session]);

  async function start(provider: OAuthProvider) {
    setPending("start");
    setError(null);
    setCompletedAccount(null);
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
    setCompletedAccount(null);
    try {
      const account = await exchangeOAuthCallback(session.sessionId, callbackUrl);
      setCompletedAccount(account);
      await queryClient.invalidateQueries({ queryKey: oauthQueryKeys.accounts });
      return account;
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
    setCompletedAccount(null);
  }

  return { session, pending, error, completedAccount, start, exchange, reset };
}

function isAbortError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "name" in error &&
    error.name === "AbortError"
  );
}
