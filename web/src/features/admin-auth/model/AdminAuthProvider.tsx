import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useState, type PropsWithChildren } from "react";

import type { AdminSessionState } from "../api/admin-auth-contracts";
import {
  getAdminSession,
  loginAdmin,
  logoutAdmin,
  rotateAdminPassword,
  setupAdmin,
} from "../api/admin-auth-api";
import { AdminAuthContext, type AdminAuthContextValue } from "./use-admin-auth";
import {
  ADMIN_SESSION_EXPIRED_EVENT,
  setAdminCsrfToken,
} from "@/shared/api/http-client";

const adminSessionKey = ["admin-auth", "session"] as const;

export function AdminAuthProvider({ children }: PropsWithChildren) {
  const queryClient = useQueryClient();
  const [submitting, setSubmitting] = useState(false);
  const operationRef = useRef(0);
  const activeOperationRef = useRef<number | null>(null);
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

  async function run<T>(action: () => Promise<T>, apply: (value: T) => void) {
    if (activeOperationRef.current !== null) {
      throw new Error("another administrator authentication action is in progress");
    }
    const operation = ++operationRef.current;
    activeOperationRef.current = operation;
    setSubmitting(true);
    try {
      const value = await action();
      if (operation === operationRef.current) {
        apply(value);
      }
    } finally {
      if (activeOperationRef.current === operation) {
        activeOperationRef.current = null;
        setSubmitting(false);
      }
    }
  }

  const value: AdminAuthContextValue = {
    session: sessionQuery.isError ? null : sessionQuery.data ?? null,
    loading: sessionQuery.isPending,
    submitting,
    error: sessionQuery.error,
    refresh: async () => {
      await sessionQuery.refetch();
    },
    setup: async (password: string, setupToken: string) => {
      await run(
        () => setupAdmin(password, setupToken),
        applySession,
      );
    },
    login: async (password: string) => {
      await run(() => loginAdmin(password), applySession);
    },
    rotatePassword: async (currentPassword: string, newPassword: string) => {
      await run(
        () => rotateAdminPassword(currentPassword, newPassword),
        applySession,
      );
    },
    logout: async () => {
      operationRef.current += 1;
      const operation = operationRef.current;
      activeOperationRef.current = operation;
      setSubmitting(true);
      try {
        try {
          await logoutAdmin();
        } catch {
          // Network/CSRF failures must not leave the console open; drop local session anyway.
        }
        if (operation === operationRef.current) {
          clearLocalAdminSession(queryClient);
        }
      } finally {
        if (activeOperationRef.current === operation) {
          activeOperationRef.current = null;
          setSubmitting(false);
        }
      }
    },
  };

  return <AdminAuthContext.Provider value={value}>{children}</AdminAuthContext.Provider>;
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
}
