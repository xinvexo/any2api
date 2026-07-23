import { useState } from "react";

import type {
  ProviderOAuthExchangeResult,
  ProviderOAuthStartInput,
  ProviderOAuthStartResult,
} from "../api/provider-credential-contracts";
import { exchangeProviderOAuth, startProviderOAuth } from "../api/provider-credential-api";

export function useProviderOAuth(endpointId: string) {
  const [session, setSession] = useState<ProviderOAuthStartResult | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function start(input: ProviderOAuthStartInput) {
    setPending(true);
    setError(null);
    let popup: Window | null = null;
    try {
      popup = window.open("about:blank", "_blank");
      if (popup) {
        popup.opener = null;
      }
      const result = await startProviderOAuth(endpointId, input);
      setSession(result);
      if (popup) {
        popup.location.href = result.authorizationUrl;
      }
      return result;
    } catch (nextError) {
      popup?.close();
      setError(nextError);
      throw nextError;
    } finally {
      setPending(false);
    }
  }

  async function exchange(callbackUrl: string): Promise<ProviderOAuthExchangeResult> {
    if (!session) {
      throw new Error("OAuth session is missing");
    }
    setPending(true);
    setError(null);
    try {
      const result = await exchangeProviderOAuth(endpointId, {
        sessionId: session.sessionId,
        callbackUrl,
      });
      setSession(null);
      return result;
    } catch (nextError) {
      setSession(null);
      setError(nextError);
      throw nextError;
    } finally {
      setPending(false);
    }
  }

  return {
    session,
    pending,
    error,
    start,
    exchange,
    reset: () => {
      setSession(null);
      setError(null);
    },
  };
}
