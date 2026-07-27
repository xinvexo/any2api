import { QueryClient } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiError } from "@/shared/api/http-client";

import { oauthQueryKeys } from "./oauth-query-keys";
import {
  deleteInspectedOAuthAccounts,
  inspectInvalidOAuthAccounts,
  isInvalidOAuthAuthenticationError,
} from "./oauth-invalid-account-cleanup";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("invalid OAuth account cleanup", () => {
  it("accepts only the explicit post-refresh authentication failure", () => {
    expect(
      isInvalidOAuthAuthenticationError(
        new ApiError(502, "oauth_account_authentication_failed", "invalid"),
      ),
    ).toBe(true);
    expect(
      isInvalidOAuthAuthenticationError(
        new ApiError(502, "oauth_account_restricted", "restricted"),
      ),
    ).toBe(false);
    expect(
      isInvalidOAuthAuthenticationError(
        new ApiError(504, "oauth_quota_timeout", "timeout"),
      ),
    ).toBe(false);
    expect(
      isInvalidOAuthAuthenticationError(
        new ApiError(
          502,
          "oauth_account_authentication_unverified",
          "refresh failed",
        ),
      ),
    ).toBe(false);
  });

  it("inspects the full batch but keeps every inconclusive account", async () => {
    const client = queryClient();
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL) => {
        const path = String(input);
        if (path.endsWith("invalid/quota")) {
          return errorResponse("oauth_account_authentication_failed", 502);
        }
        if (path.endsWith("restricted/quota")) {
          return errorResponse("oauth_account_restricted", 502);
        }
        if (path.endsWith("valid/quota")) {
          return jsonResponse(quota());
        }
        if (path === "/api/admin/oauth/accounts") {
          return jsonResponse(configuration(4, [
            account("invalid", "Invalid", 3),
            account("restricted", "Restricted", 1),
            account("valid", "Valid", 1),
          ]));
        }
        throw new Error(`unexpected request: ${path}`);
      }),
    );

    const result = await inspectInvalidOAuthAccounts(client, [
      "invalid",
      "restricted",
      "valid",
    ]);

    expect(result).toEqual({
      total: 3,
      inconclusive: 1,
      candidates: [{ id: "invalid", label: "Invalid", tokenVersion: 3 }],
    });
    expect(client.getQueryData(oauthQueryKeys.accounts)).toEqual(
      expect.objectContaining({ configRevision: 4 }),
    );
  });

  it("skips changed tokens and rechecks a revision conflict before deleting", async () => {
    const client = queryClient();
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path === "/api/admin/oauth/accounts") {
          const revision = fetchMock.mock.calls.length === 1 ? 5 : 6;
          return jsonResponse(configuration(revision, [
            account("changed", "Changed", 2),
            account("invalid", "Invalid", 1),
          ]));
        }
        if (
          path.includes("/invalid?expected_revision=5") &&
          init?.method === "DELETE"
        ) {
          return errorResponse("revision_conflict", 409);
        }
        if (
          path.includes("/invalid?expected_revision=6") &&
          init?.method === "DELETE"
        ) {
          return jsonResponse(configuration(7, [account("changed", "Changed", 2)]));
        }
        throw new Error(`unexpected request: ${path}`);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await deleteInspectedOAuthAccounts(client, [
      { id: "changed", label: "Changed", tokenVersion: 1 },
      { id: "invalid", label: "Invalid", tokenVersion: 1 },
    ]);

    expect(result).toEqual({ requested: 2, deleted: 1, skipped: 1, failed: 0 });
    const deletePaths = fetchMock.mock.calls
      .filter(([, init]) => init?.method === "DELETE")
      .map(([input]) => String(input));
    expect(deletePaths).toEqual([
      "/api/admin/oauth/accounts/invalid?expected_revision=5&expected_config_version=1",
      "/api/admin/oauth/accounts/invalid?expected_revision=6&expected_config_version=1",
    ]);
    expect(client.getQueryData(oauthQueryKeys.accounts)).toEqual(
      expect.objectContaining({ configRevision: 7 }),
    );
  });

  it("continues with later candidates after a confirmed delete failure", async () => {
    const client = queryClient();
    const fetchMock = vi.fn(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        const path = String(input);
        if (path === "/api/admin/oauth/accounts") {
          return jsonResponse(configuration(8, [
            account("first", "First", 1),
            account("second", "Second", 1),
          ]));
        }
        if (path.includes("/first?") && init?.method === "DELETE") {
          return errorResponse("oauth_delete_failed", 500);
        }
        if (path.includes("/second?") && init?.method === "DELETE") {
          return jsonResponse(configuration(9, [account("first", "First", 1)]));
        }
        throw new Error(`unexpected request: ${path}`);
      },
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await deleteInspectedOAuthAccounts(client, [
      { id: "first", label: "First", tokenVersion: 1 },
      { id: "second", label: "Second", tokenVersion: 1 },
    ]);

    expect(result).toEqual({ requested: 2, deleted: 1, skipped: 0, failed: 1 });
  });
});

function queryClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

function configuration(configRevision: number, items: unknown[]) {
  return { config_revision: configRevision, items };
}

function account(id: string, label: string, tokenVersion: number) {
  return {
    id,
    provider_kind: "codex",
    label,
    requests_per_minute: null,
    enabled: true,
    safe_account_email: null,
    expires_at: null,
    token_version: tokenVersion,
    account_generation: tokenVersion,
    config_version: 1,
    selected_model_count: 0,
    models: [],
    available_models: ["gpt-5.5"],
    plan_type: "free",
    bot_flagged: null,
    usage: usage(),
  };
}

function quota() {
  return { fetched_at: 1_900_000_000, rate_limit: null, reset_credits: null };
}

function errorResponse(code: string, status: number) {
  return new Response(
    JSON.stringify({ error: { code, message: "request failed" } }),
    { status, headers: { "Content-Type": "application/json" } },
  );
}

function jsonResponse(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function usage() {
  const intervalMs = 2 * 60 * 1_000;
  return {
    total_requests: 0,
    successful_requests: 0,
    failed_requests: 0,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: index * intervalMs,
      total_requests: 0,
      successful_requests: 0,
      failed_requests: 0,
    })),
  };
}
