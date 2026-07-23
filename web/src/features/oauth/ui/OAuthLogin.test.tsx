import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  expect(screen.getByRole("button", { name: "下载 JSON" })).toBeDisabled();
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

function renderLogin(initialEntries: string[] = ["/oauth"]) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <OAuthLogin />
    </MemoryRouter>,
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
