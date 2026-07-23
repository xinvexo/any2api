import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { AdminSessionState } from "../api/admin-auth-contracts";
import {
  getAdminSession,
  loginAdmin,
  logoutAdmin,
  rotateAdminPassword,
  setupAdmin,
} from "../api/admin-auth-api";
import {
  ADMIN_SESSION_EXPIRED_EVENT,
  setAdminCsrfToken,
} from "@/shared/api/http-client";

const adminSessionKey = ["admin-auth", "session"] as const;

export function useAdminAuth() {
  const queryClient = useQueryClient();
  const [submitting, setSubmitting] = useState(false);
  const sessionQuery = useQuery({
    queryKey: adminSessionKey,
    queryFn: ({ signal }) => getAdminSession(signal),
    retry: false,
    staleTime: 0,
  });

  useEffect(() => {
    setAdminCsrfToken(sessionQuery.data?.csrfToken ?? null);
  }, [sessionQuery.data]);

  useEffect(() => {
    const handleExpired = () => {
      clearLocalAdminSession(queryClient);
    };
    window.addEventListener(ADMIN_SESSION_EXPIRED_EVENT, handleExpired);
    return () => window.removeEventListener(ADMIN_SESSION_EXPIRED_EVENT, handleExpired);
  }, [queryClient]);

  const applySession = (session: AdminSessionState) => {
    setAdminCsrfToken(session.csrfToken);
    queryClient.setQueryData(adminSessionKey, session);
  };
  async function run(action: () => Promise<void>) {
    setSubmitting(true);
    try {
      await action();
    } finally {
      setSubmitting(false);
    }
  }

  return {
    session: sessionQuery.isError ? null : sessionQuery.data ?? null,
    loading: sessionQuery.isPending,
    submitting,
    error: sessionQuery.error,
    refresh: async () => {
      await sessionQuery.refetch();
    },
    setup: async (password: string, setupToken: string) => {
      await run(async () => applySession(await setupAdmin(password, setupToken)));
    },
    login: async (password: string) => {
      await run(async () => applySession(await loginAdmin(password)));
    },
    rotatePassword: async (currentPassword: string, newPassword: string) => {
      await run(async () =>
        applySession(await rotateAdminPassword(currentPassword, newPassword)),
      );
    },
    logout: async () => {
      await run(async () => {
        try {
          await logoutAdmin();
        } catch {
          // Network/CSRF failures must not leave the console open; drop local session anyway.
        }
        clearLocalAdminSession(queryClient);
      });
    },
  };
}

/** Drop non-session caches and force the gate back to the login screen. */
function clearLocalAdminSession(queryClient: QueryClient) {
  const current = queryClient.getQueryData<AdminSessionState>(adminSessionKey);
  queryClient.removeQueries({
    predicate: (query) => query.queryKey[0] !== adminSessionKey[0],
  });
  queryClient.getMutationCache().clear();
  setAdminCsrfToken(null);
  queryClient.setQueryData<AdminSessionState>(adminSessionKey, {
    initialized: current?.initialized ?? true,
    authenticated: false,
    csrfToken: null,
    remoteAccessEnabled: current?.remoteAccessEnabled ?? false,
    secureTransport: current?.secureTransport ?? false,
    clientLoopback: current?.clientLoopback ?? true,
    throughTrustedProxy: current?.throughTrustedProxy ?? false,
    plaintextHttpWarning: current?.plaintextHttpWarning ?? false,
  });
  void queryClient.invalidateQueries({ queryKey: adminSessionKey });
}
