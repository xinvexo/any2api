import { useRef, useState, type RefObject } from "react";

import { testProviderCredential } from "../api/provider-credential-api";
import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import type { ProviderEndpoint } from "../api/provider-contracts";
import type { ProxyConfiguration } from "@/features/proxies";

export function useProviderCredentialTest(configurationScope: string) {
  const [state, setState] = useState<TestState>(() => emptyState(configurationScope));
  const scope = useRef(configurationScope);
  const sequence = useRef(0);
  if (scope.current !== configurationScope) {
    sequence.current += 1;
  }
  scope.current = configurationScope;
  const visible = state.scope === configurationScope
    ? state
    : emptyState(configurationScope);

  async function test(credentialId: string) {
    const startedScope = scope.current;
    const requestId = ++sequence.current;
    setState((current) => ({
      scope: startedScope,
      testingCredentialId: credentialId,
      results: removeResult(current, startedScope, credentialId),
      error: null,
    }));
    try {
      const result = await testProviderCredential(credentialId);
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({
          ...current,
          results: { ...current.results, [credentialId]: result },
        }));
        return result;
      }
    } catch (nextError) {
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({ ...current, error: nextError }));
      }
    } finally {
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({ ...current, testingCredentialId: null }));
      }
    }
    return undefined;
  }

  return {
    testingCredentialId: visible.testingCredentialId,
    results: visible.results,
    error: visible.error,
    test,
  };
}

interface TestState {
  scope: string;
  testingCredentialId: string | null;
  results: Record<string, ProviderCredentialTestResult>;
  error: unknown;
}

function emptyState(scope: string): TestState {
  return { scope, testingCredentialId: null, results: {}, error: null };
}

function removeResult(current: TestState, scope: string, credentialId: string) {
  if (current.scope !== scope) {
    return {};
  }
  const results = { ...current.results };
  delete results[credentialId];
  return results;
}

function isActive(
  scope: RefObject<string>,
  sequence: RefObject<number>,
  startedScope: string,
  requestId: number,
) {
  return scope.current === startedScope && sequence.current === requestId;
}

export function providerCredentialTestScope(
  endpoint: ProviderEndpoint,
  credential: ProviderCredential | undefined,
  proxies: ProxyConfiguration | undefined,
) {
  const endpointScope = `endpoint:${endpoint.id}:${endpoint.configVersion}`;
  if (!credential || !proxies) {
    return `${endpointScope}:credential:${credential?.id ?? "none"}:unresolved`;
  }

  const boundProxy = proxies.items.find((proxy) => proxy.id === credential.proxyProfileId);
  const effectiveProxyId = boundProxy?.kind === "direct"
    ? proxies.globalProxyId
    : credential.proxyProfileId;
  const effectiveProxy = proxies.items.find((proxy) => proxy.id === effectiveProxyId);

  return [
    endpointScope,
    "credential",
    credential.id,
    credential.configVersion,
    credential.credentialGeneration,
    credential.secretVersion,
    credential.proxyProfileId,
    "proxy",
    effectiveProxyId,
    effectiveProxy?.configVersion ?? "missing",
    effectiveProxy?.authenticationVersion ?? "missing",
  ].join(":");
}
