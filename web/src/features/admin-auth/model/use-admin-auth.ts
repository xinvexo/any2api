import { createContext, useContext } from "react";

import type { AdminSessionState } from "../api/admin-auth-contracts";

export interface AdminAuthContextValue {
  session: AdminSessionState | null;
  loading: boolean;
  submitting: boolean;
  error: unknown;
  refresh: () => Promise<void>;
  setup: (password: string, setupToken: string) => Promise<void>;
  login: (password: string) => Promise<void>;
  rotatePassword: (currentPassword: string, newPassword: string) => Promise<void>;
  logout: () => Promise<void>;
}

export const AdminAuthContext = createContext<AdminAuthContextValue | null>(null);

export function useAdminAuth() {
  const value = useContext(AdminAuthContext);
  if (!value) {
    throw new Error("useAdminAuth must be used within AdminAuthProvider");
  }
  return value;
}
