import { useRef, useState, type RefObject } from "react";

import { testProxy } from "../api/proxy-api";
import type { ProxyTestResult } from "../api/proxy-contracts";

export function useProxyTest(configurationScope: string) {
  const [state, setState] = useState<TestState>(() => emptyState(configurationScope));
  const scope = useRef(configurationScope);
  const sequence = useRef(0);
  scope.current = configurationScope;
  const visible = state.scope === configurationScope
    ? state
    : emptyState(configurationScope);

  async function test(proxyId: string, providerEndpointId: string) {
    const startedScope = scope.current;
    const requestId = ++sequence.current;
    setState((current) => ({
      scope: startedScope,
      testingProxyId: proxyId,
      results: removeResult(current, startedScope, proxyId),
      error: null,
      errorProxyId: null,
    }));

    try {
      const result = await testProxy(proxyId, providerEndpointId);
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({
          ...current,
          results: { ...current.results, [proxyId]: result },
        }));
      }
    } catch (error) {
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({ ...current, error, errorProxyId: proxyId }));
      }
    } finally {
      if (isActive(scope, sequence, startedScope, requestId)) {
        setState((current) => ({ ...current, testingProxyId: null }));
      }
    }
  }

  return {
    testingProxyId: visible.testingProxyId,
    results: visible.results,
    error: visible.error,
    errorProxyId: visible.errorProxyId,
    test,
  };
}

interface TestState {
  scope: string;
  testingProxyId: string | null;
  results: Record<string, ProxyTestResult>;
  error: unknown;
  errorProxyId: string | null;
}

function emptyState(scope: string): TestState {
  return {
    scope,
    testingProxyId: null,
    results: {},
    error: null,
    errorProxyId: null,
  };
}

function removeResult(current: TestState, scope: string, proxyId: string) {
  if (current.scope !== scope) {
    return {};
  }
  const results = { ...current.results };
  delete results[proxyId];
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
