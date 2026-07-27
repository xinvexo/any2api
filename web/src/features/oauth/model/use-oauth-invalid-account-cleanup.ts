import { useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import {
  deleteInspectedOAuthAccounts,
  inspectInvalidOAuthAccounts,
  type InvalidOAuthAccountCandidate,
} from "./oauth-invalid-account-cleanup";

export type OAuthInvalidCleanupPhase = "inspecting" | "deleting" | null;

export function useOAuthInvalidAccountCleanup() {
  const queryClient = useQueryClient();
  const pendingRef = useRef(false);
  const [phase, setPhase] = useState<OAuthInvalidCleanupPhase>(null);

  async function run<T>(
    nextPhase: Exclude<OAuthInvalidCleanupPhase, null>,
    operation: () => Promise<T>,
  ): Promise<T | null> {
    if (pendingRef.current) {
      return null;
    }
    pendingRef.current = true;
    setPhase(nextPhase);
    try {
      return await operation();
    } finally {
      pendingRef.current = false;
      setPhase(null);
    }
  }

  const inspect = (accountIds: readonly string[]) =>
    run("inspecting", () => inspectInvalidOAuthAccounts(queryClient, accountIds));
  const remove = (candidates: readonly InvalidOAuthAccountCandidate[]) =>
    run("deleting", () => deleteInspectedOAuthAccounts(queryClient, candidates));

  return { phase, pending: phase !== null, inspect, remove };
}
