import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { OAuthImportDrawer } from "./OAuthImport";
import { setAdminCsrfToken } from "@/shared/api/http-client";

afterEach(() => {
  setAdminCsrfToken(null);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("uploads multiple files as FormData with CSRF and clears local file state", async () => {
  const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    void input;
    void init;
    return jsonResponse({
      imported_count: 2,
      config_revision: 2,
      items: [imported("one", "codex"), imported("two", "claude")],
    });
  });
  vi.stubGlobal("fetch", fetchMock);
  setAdminCsrfToken("csrf-token");
  const onImported = vi.fn();
  const onClose = vi.fn();
  render(<OAuthImportDrawer onClose={onClose} onImported={onImported} />);
  const files = [
    new File(["{\"type\":\"codex\"}"], "codex.json", {
      type: "application/json",
    }),
    new File(["{\"type\":\"claude\"}"], "claude.json", {
      type: "application/json",
    }),
  ];

  fireEvent.change(screen.getByLabelText("OAuth JSON 文件"), {
    target: { files },
  });
  expect(screen.getByText(/已选择/)).toHaveTextContent("2 个文件");
  fireEvent.click(screen.getByRole("button", { name: "导入并启用" }));

  await waitFor(() => expect(onImported).toHaveBeenCalledTimes(1));
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(screen.queryByText(/已选择/)).not.toBeInTheDocument();
  const [, init] = fetchMock.mock.calls[0] ?? [];
  expect(init?.method).toBe("POST");
  expect(init?.body).toBeInstanceOf(FormData);
  const form = init?.body as FormData;
  expect(form.getAll("files")).toHaveLength(2);
  expect((form.getAll("files")[0] as File).name).toBe("codex.json");
  const headers = init?.headers as Record<string, string>;
  expect(headers["X-CSRF-Token"]).toBe("csrf-token");
  expect(headers["Content-Type"]).toBeUndefined();
});

test("clears selected Files after failure and on close", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      new Response(
        JSON.stringify({
          error: {
            code: "oauth_import_invalid_account",
            message: "unsafe provider detail",
          },
        }),
        { status: 400, headers: { "Content-Type": "application/json" } },
      ),
    ),
  );
  const onClose = vi.fn();
  render(<OAuthImportDrawer onClose={onClose} onImported={vi.fn()} />);
  const input = screen.getByLabelText("OAuth JSON 文件");
  fireEvent.change(input, {
    target: { files: [new File(["{}"], "bad.json", { type: "application/json" })] },
  });
  fireEvent.click(screen.getByRole("button", { name: "导入并启用" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "JSON 中的 OAuth 认证信息无效。",
  );
  expect(screen.queryByText(/已选择/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "导入并启用" })).toBeDisabled();

  fireEvent.change(input, {
    target: { files: [new File(["{}"], "again.json", { type: "application/json" })] },
  });
  expect(screen.getByText(/已选择/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "关闭" }));
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(screen.queryByText(/已选择/)).not.toBeInTheDocument();
});

function imported(id: string, provider: "codex" | "claude") {
  return {
    id,
    provider_kind: provider,
    label: `${provider} imported`,
    requests_per_minute: null,
    proxy_selection: { mode: "global" },
    enabled: true,
    safe_account_email: null,
    expires_at: null,
    selected_model_count: 1,
    config_version: 1,
  };
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}
