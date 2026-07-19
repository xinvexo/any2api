import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AppProviders } from "@/app/providers";
import { ADMIN_SESSION_EXPIRED_EVENT, setAdminCsrfToken } from "@/shared/api/http-client";

import { AdminAuthGate } from "./AdminAuthGate";
import { AdminSecurityBanner } from "./AdminSecurityBanner";

afterEach(() => {
  vi.unstubAllGlobals();
  setAdminCsrfToken(null);
});

test("local first run completes setup and enters the protected application", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/auth/session") {
      return jsonResponse(session(false, false, null));
    }
    if (path === "/api/admin/auth/setup" && init?.method === "POST") {
      return jsonResponse(session(true, true, "csrf-token"));
    }
    throw new Error(`unexpected request ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  render(
    <AppProviders>
      <AdminAuthGate>
        <p>protected console</p>
      </AdminAuthGate>
    </AppProviders>,
  );

  expect(await screen.findByRole("heading", { name: "初始化管理员" })).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("Setup Token"), {
    target: { value: "setup-token" },
  });
  fireEvent.change(screen.getByLabelText("管理员密码"), {
    target: { value: "correct horse battery staple" },
  });
  fireEvent.change(screen.getByLabelText("确认密码"), {
    target: { value: "correct horse battery staple" },
  });
  fireEvent.click(screen.getByRole("button", { name: "创建管理员" }));

  expect(await screen.findByText("protected console")).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.some(
      ([input, init]) => String(input) === "/api/admin/auth/setup" && init?.method === "POST",
    ),
  ).toBe(true);
});

test("authenticated remote HTTP keeps the security warning visible", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      jsonResponse({
        ...session(true, true, "csrf-token"),
        remote_access_enabled: true,
        client_loopback: false,
        plaintext_http_warning: true,
      }),
    ),
  );

  render(
    <AppProviders>
      <AdminAuthGate>
        <AdminSecurityBanner />
      </AdminAuthGate>
    </AppProviders>,
  );

  expect(await screen.findByText(/当前远程管理使用明文 HTTP/)).toBeInTheDocument();
});

test("remote HTTP login warns before the password is submitted", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      jsonResponse({
        ...session(true, false, null),
        remote_access_enabled: true,
        client_loopback: false,
        plaintext_http_warning: true,
      }),
    ),
  );

  render(
    <AppProviders>
      <AdminAuthGate>
        <p>protected console</p>
      </AdminAuthGate>
    </AppProviders>,
  );

  expect(await screen.findByRole("heading", { name: "管理员登录" })).toBeInTheDocument();
  expect(screen.getByText(/当前连接使用明文 HTTP/)).toBeInTheDocument();
});

test("session expiry immediately closes the protected view", async () => {
  let authenticated = true;
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      jsonResponse(session(true, authenticated, authenticated ? "csrf-token" : null)),
    ),
  );

  render(
    <AppProviders>
      <AdminAuthGate>
        <p>protected console</p>
      </AdminAuthGate>
    </AppProviders>,
  );
  expect(await screen.findByText("protected console")).toBeInTheDocument();

  authenticated = false;
  window.dispatchEvent(new Event(ADMIN_SESSION_EXPIRED_EVENT));
  expect(await screen.findByRole("heading", { name: "管理员登录" })).toBeInTheDocument();
  expect(screen.queryByText("protected console")).not.toBeInTheDocument();
});

function session(initialized: boolean, authenticated: boolean, csrfToken: string | null) {
  return {
    initialized,
    authenticated,
    csrf_token: csrfToken,
    remote_access_enabled: false,
    secure_transport: false,
    client_loopback: true,
    through_trusted_proxy: false,
    plaintext_http_warning: false,
  };
}

function jsonResponse(value: unknown) {
  return new Response(JSON.stringify(value), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
