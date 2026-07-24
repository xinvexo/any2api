import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import {
  exchangeOAuthCallback,
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
  const [pending, setPending] = useState<"start" | "exchange" | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [completedAccount, setCompletedAccount] =
    useState<OAuthActivationResult | null>(null);

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
