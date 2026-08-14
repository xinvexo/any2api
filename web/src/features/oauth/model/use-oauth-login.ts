import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import {
  exchangeOAuthCallback,
  pollOAuthDevice,
  startOAuthLogin,
} from "../api/oauth-api";
import type {
  OAuthActivationResult,
  OAuthProvider,
  OAuthProxySelection,
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
  const generationRef = useRef(0);
  const actionControllerRef = useRef<AbortController | null>(null);

  function beginGeneration() {
    actionControllerRef.current?.abort();
    generationRef.current += 1;
    setSession(null);
    setError(null);
    setCompletedAccount(null);
    return generationRef.current;
  }

  function isCurrent(generation: number) {
    return generationRef.current === generation;
  }

  useEffect(() => {
    if (session?.flow !== "device_code") {
      return;
    }
    const controller = new AbortController();
    actionControllerRef.current = controller;
    const generation = generationRef.current;
    let timer: number | null = null;

    const schedule = (delaySeconds: number) => {
      timer = window.setTimeout(() => void poll(), delaySeconds * 1_000);
    };
    const poll = async () => {
      setPending("poll");
      try {
        const result = await pollOAuthDevice(session.sessionId, controller.signal);
        if (controller.signal.aborted || !isCurrent(generation)) {
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
        if (controller.signal.aborted || !isCurrent(generation) || isAbortError(nextError)) {
          return;
        }
        await reconcileAccounts(queryClient);
        if (!isCurrent(generation)) {
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
      if (actionControllerRef.current === controller) {
        actionControllerRef.current = null;
      }
      if (timer !== null) {
        window.clearTimeout(timer);
      }
    };
  }, [queryClient, session]);

  async function start(
    provider: OAuthProvider,
    proxySelection: OAuthProxySelection,
  ) {
    const generation = beginGeneration();
    const controller = new AbortController();
    actionControllerRef.current = controller;
    setPending("start");
    try {
      const result = await startOAuthLogin(provider, proxySelection, controller.signal);
      if (!isCurrent(generation)) {
        return result;
      }
      setSession(result);
      return result;
    } catch (nextError) {
      if (isCurrent(generation) && !isAbortError(nextError)) {
        setError(nextError);
      }
      throw nextError;
    } finally {
      if (isCurrent(generation)) {
        setPending(null);
      }
      if (actionControllerRef.current === controller) {
        actionControllerRef.current = null;
      }
    }
  }

  async function exchange(callbackUrl: string) {
    if (!session) {
      return;
    }
    const generation = generationRef.current;
    const controller = new AbortController();
    actionControllerRef.current = controller;
    setPending("exchange");
    setError(null);
    setCompletedAccount(null);
    try {
      const account = await exchangeOAuthCallback(
        session.sessionId,
        callbackUrl,
        controller.signal,
      );
      if (!isCurrent(generation)) {
        return;
      }
      setCompletedAccount(account);
      await queryClient.invalidateQueries({ queryKey: oauthQueryKeys.accounts });
      return account;
    } catch (nextError) {
      if (!controller.signal.aborted && isCurrent(generation) && !isAbortError(nextError)) {
        await reconcileAccounts(queryClient);
        if (isCurrent(generation)) {
          setError(nextError);
        }
      }
      throw nextError;
    } finally {
      if (isCurrent(generation)) {
        setSession(null);
        setPending(null);
      }
      if (actionControllerRef.current === controller) {
        actionControllerRef.current = null;
      }
    }
  }

  function reset() {
    beginGeneration();
    setPending(null);
  }

  return { session, pending, error, completedAccount, start, exchange, reset };
}

async function reconcileAccounts(queryClient: QueryClient) {
  try {
    await queryClient.invalidateQueries({
      queryKey: oauthQueryKeys.accounts,
      refetchType: "active",
    });
  } catch {
    // Preserve the original control-plane error when the reconciliation read also fails.
  }
}

function isAbortError(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "name" in error &&
    error.name === "AbortError"
  );
}
