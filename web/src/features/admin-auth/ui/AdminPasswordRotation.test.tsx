import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, expect, test, vi } from "vitest";

import { ADMIN_SESSION_EXPIRED_EVENT, setAdminCsrfToken } from "@/shared/api/http-client";

import { AdminPasswordRotation } from "./AdminPasswordRotation";

afterEach(() => {
  vi.unstubAllGlobals();
  setAdminCsrfToken(null);
});

test("rotates the password with the in-memory CSRF token and refreshes the session", async () => {
  let rotationInit: RequestInit | undefined;
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const path = String(input);
      if (path === "/api/admin/auth/session") {
        return jsonResponse(session("old-csrf"));
      }
      if (path === "/api/admin/auth/password/rotate") {
        rotationInit = init;
        return jsonResponse(session("new-csrf"));
      }
      throw new Error(`unexpected request ${path}`);
    }),
  );

  const client = renderRotation();

  await screen.findByRole("heading", { name: "管理员密码" });
  fireEvent.change(screen.getByLabelText("当前密码"), {
    target: { value: "correct horse battery staple" },
  });
  fireEvent.change(screen.getByLabelText("新密码"), {
    target: { value: "new correct horse battery staple" },
  });
  fireEvent.change(screen.getByLabelText("确认新密码"), {
    target: { value: "new correct horse battery staple" },
  });
  fireEvent.click(screen.getByRole("button", { name: "更新密码" }));

  expect(await screen.findByText("密码已更新，当前会话已刷新。")).toBeInTheDocument();
  expect(rotationInit?.headers).toMatchObject({ "X-CSRF-Token": "old-csrf" });
  expect(JSON.parse(String(rotationInit?.body))).toEqual({
    current_password: "correct horse battery staple",
    new_password: "new correct horse battery staple",
  });
  expect(screen.getByLabelText("当前密码")).toHaveValue("");
  expect(screen.getByLabelText("新密码")).toHaveValue("");
  expect(screen.getByLabelText("确认新密码")).toHaveValue("");
  expect(JSON.stringify(client.getQueryData(["admin-auth", "session"]))).not.toContain(
    "correct horse battery staple",
  );
  expect(JSON.stringify(client.getMutationCache().getAll())).not.toContain(
    "correct horse battery staple",
  );
});

test("current password errors do not expire the administrator session", async () => {
  let expired = false;
  const onExpired = () => {
    expired = true;
  };
  window.addEventListener(ADMIN_SESSION_EXPIRED_EVENT, onExpired);
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input);
      if (path === "/api/admin/auth/session") {
        return jsonResponse(session("csrf"));
      }
      return new Response(
        JSON.stringify({
          error: {
            code: "admin_current_password_invalid",
            message: "the current administrator password is invalid",
          },
        }),
        { status: 403, headers: { "Content-Type": "application/json" } },
      );
    }),
  );

  renderRotation();
  await screen.findByRole("heading", { name: "管理员密码" });
  fireEvent.change(screen.getByLabelText("当前密码"), { target: { value: "old" } });
  fireEvent.change(screen.getByLabelText("新密码"), { target: { value: "new password value" } });
  fireEvent.change(screen.getByLabelText("确认新密码"), { target: { value: "new password value" } });
  fireEvent.click(screen.getByRole("button", { name: "更新密码" }));

  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent("当前管理员密码不正确。"),
  );
  expect(expired).toBe(false);
  expect(screen.getByLabelText("当前密码")).toHaveValue("");
  window.removeEventListener(ADMIN_SESSION_EXPIRED_EVENT, onExpired);
});

function session(csrfToken: string) {
  return {
    initialized: true,
    authenticated: true,
    csrf_token: csrfToken,
    remote_access_enabled: false,
    secure_transport: true,
    client_loopback: false,
    through_trusted_proxy: true,
    plaintext_http_warning: false,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function renderRotation() {
  const client = new QueryClient();
  render(
    <QueryClientProvider client={client}>
      <AdminPasswordRotation />
    </QueryClientProvider>,
  );
  return client;
}
