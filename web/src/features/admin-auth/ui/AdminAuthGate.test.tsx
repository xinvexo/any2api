import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AppProviders } from "@/app/providers";
import { ADMIN_SESSION_EXPIRED_EVENT, setAdminCsrfToken } from "@/shared/api/http-client";

import { useAdminAuth } from "../model/use-admin-auth";

import { AdminAuthGate } from "./AdminAuthGate";
import { AdminSecurityBanner } from "./AdminSecurityBanner";

afterEach(() => {
  vi.restoreAllMocks();
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

  expect(await screen.findByRole("heading", { name: "any2api" })).toBeInTheDocument();
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

  expect(await screen.findByRole("heading", { name: "any2api" })).toBeInTheDocument();
  expect(screen.getByText(/当前连接使用明文 HTTP/)).toBeInTheDocument();
});

test("login input supports the browser password manager", async () => {
  const storageWrites = vi.spyOn(Storage.prototype, "setItem");
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/auth/session") {
      return jsonResponse(session(true, false, null));
    }
    if (path === "/api/admin/auth/login" && init?.method === "POST") {
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

  expect(await screen.findByRole("heading", { name: "any2api" })).toBeInTheDocument();
  const password = screen.getByLabelText("管理员密码");
  expect(password).toHaveValue("");
  expect(password).toHaveAttribute("autocomplete", "current-password");
  expect(screen.queryByRole("checkbox", { name: "记住密码" })).not.toBeInTheDocument();

  const submittedPassword = "secret-admin-password";
  fireEvent.change(password, {
    target: { value: submittedPassword },
  });
  fireEvent.click(screen.getByRole("button", { name: "进入控制台" }));

  expect(await screen.findByText("protected console")).toBeInTheDocument();
  expect(storageWrites.mock.calls.flat().join("\n")).not.toContain(submittedPassword);
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
  expect(await screen.findByRole("heading", { name: "any2api" })).toBeInTheDocument();
  expect(screen.queryByText("protected console")).not.toBeInTheDocument();
});

test("logout returns to the login screen", async () => {
  let authenticated = true;
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input);
    if (path === "/api/admin/auth/session") {
      return jsonResponse(session(true, authenticated, authenticated ? "csrf-token" : null));
    }
    if (path === "/api/admin/auth/logout" && init?.method === "POST") {
      authenticated = false;
      return new Response(null, { status: 204 });
    }
    throw new Error(`unexpected request ${path}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  render(
    <AppProviders>
      <AdminAuthGate>
        <LogoutProbe />
      </AdminAuthGate>
    </AppProviders>,
  );

  expect(await screen.findByText("protected console")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "退出" }));

  expect(await screen.findByRole("heading", { name: "any2api" })).toBeInTheDocument();
  expect(screen.queryByText("protected console")).not.toBeInTheDocument();
  expect(screen.getByLabelText("管理员密码")).toBeInTheDocument();
  expect(
    fetchMock.mock.calls.some(
      ([input, init]) => String(input) === "/api/admin/auth/logout" && init?.method === "POST",
    ),
  ).toBe(true);
});

function LogoutProbe() {
  const auth = useAdminAuth();
  return (
    <div>
      <p>protected console</p>
      <button type="button" onClick={() => void auth.logout()}>
        退出
      </button>
    </div>
  );
}

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
