import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthLogin } from "./OAuthLogin";

afterEach(() => vi.restoreAllMocks());

test("renders left provider categories and empty state actions", () => {
  renderLogin();

  expect(screen.getByRole("navigation", { name: "OAuth2 类型" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /Codex/ })).toHaveAttribute("aria-current", "page");
  expect(screen.getByRole("button", { name: /Claude/ })).toBeInTheDocument();
  expect(screen.getByText("还没有 Codex 登录会话")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
});

test("switches provider category via the left nav", () => {
  renderLogin();

  fireEvent.click(screen.getByRole("button", { name: /Claude/ }));

  expect(screen.getByRole("button", { name: /Claude/ })).toHaveAttribute("aria-current", "page");
  expect(screen.getByText("还没有 Claude 登录会话")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
});

test("starts a login session and shows callback form", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    if (String(input) === "/api/admin/oauth/start" && init?.method === "POST") {
      return jsonResponse({
        provider: "codex",
        session_id: "session-1",
        authorization_url: "https://auth.example/authorize?state=abc",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      });
    }
    throw new Error(`unexpected request: ${String(input)}`);
  });
  vi.stubGlobal("fetch", fetchMock);

  renderLogin();
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));

  expect(await screen.findByText("Codex 授权会话")).toBeInTheDocument();
  expect(screen.getByText("http://localhost:1455/auth/callback")).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开授权页" })).toHaveAttribute(
    "href",
    "https://auth.example/authorize?state=abc",
  );
  expect(screen.getByLabelText("回调 URL")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "激活账号" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "重新开始" })).toBeInTheDocument();

  await waitFor(() => {
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/admin/oauth/start",
      expect.objectContaining({ method: "POST" }),
    );
  });
  expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
    provider: "codex",
  });
});

test("switching provider resets an in-progress session", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      jsonResponse({
        provider: "codex",
        session_id: "session-1",
        authorization_url: "https://auth.example/authorize",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      }),
    ),
  );

  renderLogin();
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));
  expect(await screen.findByText("Codex 授权会话")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: /Claude/ }));

  expect(screen.queryByText("Codex 授权会话")).not.toBeInTheDocument();
  expect(screen.getByText("还没有 Claude 登录会话")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
});

test("activates an account without downloading token material", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    if (String(input) === "/api/admin/oauth/start") {
      return jsonResponse({
        provider: "codex",
        session_id: "session-1",
        authorization_url: "https://auth.example/authorize?state=abc",
        redirect_uri: "http://localhost:1455/auth/callback",
        expires_in_seconds: 600,
      });
    }
    if (String(input) === "/api/admin/oauth/exchange" && init?.method === "POST") {
      return jsonResponse({
        provider: "codex",
        account_id: "fdcb6e74-820f-4d84-9df6-38af2b031feb",
        label: "Codex OAuth fdcb6e74-820f-4d84-9df6-38af2b031feb",
        max_concurrency: 1,
        enabled: true,
        safe_account_email: null,
        expires_at: 1_800_000_000,
        selected_model_count: 8,
        config_version: 1,
        config_revision: 2,
      });
    }
    throw new Error("unexpected request: " + String(input));
  });
  vi.stubGlobal("fetch", fetchMock);

  renderLogin();
  fireEvent.click(screen.getByRole("button", { name: "OAuth认证" }));
  await screen.findByText("Codex 授权会话");
  fireEvent.change(screen.getByLabelText("回调 URL"), {
    target: { value: "http://localhost:1455/auth/callback?code=abc&state=abc" },
  });
  fireEvent.click(screen.getByRole("button", { name: "激活账号" }));

  expect(
    await screen.findByText(/已激活 Codex OAuth fdcb6e74.*已选择 8 个模型/),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "OAuth认证" })).toBeInTheDocument();
  const exchangeCall = fetchMock.mock.calls.find(
    ([input]) => String(input) === "/api/admin/oauth/exchange",
  );
  expect(JSON.parse(String(exchangeCall?.[1]?.body))).toEqual({
    session_id: "session-1",
    callback_url: "http://localhost:1455/auth/callback?code=abc&state=abc",
  });
});

function renderLogin(initialEntries: string[] = ["/oauth"]) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <MemoryRouter initialEntries={initialEntries}>
        <OAuthLogin />
      </MemoryRouter>
    </QueryClientProvider>,
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
