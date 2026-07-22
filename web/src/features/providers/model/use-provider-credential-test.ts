import { useRef, useState } from "react";

import { testProviderCredential } from "../api/provider-credential-api";
import type { ProviderCredentialTestResult } from "../api/provider-credential-contracts";

export function useProviderCredentialTest(configurationScope: string) {
  const [state, setState] = useState<TestState>(() => emptyState(configurationScope));
  const scope = useRef(configurationScope);
  scope.current = configurationScope;
  const visible = state.scope === configurationScope
    ? state
    : emptyState(configurationScope);

  async function test(credentialId: string) {
    const startedScope = scope.current;
    setState((current) => ({
      scope: startedScope,
      testingCredentialId: credentialId,
      results: removeResult(current, startedScope, credentialId),
      error: null,
    }));
    try {
      const result = await testProviderCredential(credentialId);
      if (scope.current === startedScope) {
        setState((current) => ({
          ...current,
          results: { ...current.results, [credentialId]: result },
        }));
      }
    } catch (nextError) {
      if (scope.current === startedScope) {
        setState((current) => ({ ...current, error: nextError }));
      }
    } finally {
      if (scope.current === startedScope) {
        setState((current) => ({ ...current, testingCredentialId: null }));
      }
    }
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
