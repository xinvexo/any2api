import { requestJson } from "@/shared/api/http-client";

import { parseAdminSessionState } from "./admin-auth-contracts";

export function getAdminSession(signal?: AbortSignal) {
  return requestJson<unknown>("/api/admin/auth/session", { signal }).then(parseAdminSessionState);
}

export function setupAdmin(password: string, setupToken: string) {
  return requestJson<unknown>("/api/admin/auth/setup", {
    method: "POST",
    body: { setup_token: setupToken, password },
  }).then(parseAdminSessionState);
}

export function loginAdmin(password: string) {
  return requestJson<unknown>("/api/admin/auth/login", {
    method: "POST",
    body: { password },
  }).then(parseAdminSessionState);
}

export function logoutAdmin() {
  return requestJson<void>("/api/admin/auth/logout", { method: "POST" });
}

export function rotateAdminPassword(currentPassword: string, newPassword: string) {
  return requestJson<unknown>("/api/admin/auth/password/rotate", {
    method: "POST",
    body: { current_password: currentPassword, new_password: newPassword },
  }).then(parseAdminSessionState);
}
