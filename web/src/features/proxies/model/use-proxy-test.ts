import { useRef, useState } from "react";

import { testProxy } from "../api/proxy-api";
import type { ProxyTestResult } from "../api/proxy-contracts";

export function useProxyTest(configurationScope: string) {
  const [state, setState] = useState<TestState>(() => emptyState(configurationScope));
  const scope = useRef(configurationScope);
  scope.current = configurationScope;
  const visible = state.scope === configurationScope
    ? state
    : emptyState(configurationScope);

  async function test(proxyId: string, providerEndpointId: string) {
    const startedScope = scope.current;
    setState((current) => ({
      scope: startedScope,
      testingProxyId: proxyId,
      results: removeResult(current, startedScope, proxyId),
      error: null,
    }));
    try {
      const result = await testProxy(proxyId, providerEndpointId);
      if (scope.current === startedScope) {
        setState((current) => ({
          ...current,
          results: { ...current.results, [proxyId]: result },
        }));
      }
    } catch (nextError) {
      if (scope.current === startedScope) {
        setState((current) => ({ ...current, error: nextError }));
      }
    } finally {
      if (scope.current === startedScope) {
        setState((current) => ({ ...current, testingProxyId: null }));
      }
    }
  }

  return {
    testingProxyId: visible.testingProxyId,
    results: visible.results,
    error: visible.error,
    test,
  };
}

interface TestState {
  scope: string;
  testingProxyId: string | null;
  results: Record<string, ProxyTestResult>;
  error: unknown;
}

function emptyState(scope: string): TestState {
  return { scope, testingProxyId: null, results: {}, error: null };
}

function removeResult(current: TestState, scope: string, proxyId: string) {
  if (current.scope !== scope) {
    return {};
  }
  const results = { ...current.results };
  delete results[proxyId];
  return results;
}
