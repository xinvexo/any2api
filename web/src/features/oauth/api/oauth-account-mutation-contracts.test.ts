import { describe, expect, it } from "vitest";

import { parseOAuthAccountConfiguration } from "./oauth-contracts";
import { parseOAuthAccountMutationResponse } from "./oauth-account-mutation-contracts";
import { mergeOAuthAccountMutationResponse } from "../model/merge-oauth-account-mutation-response";

describe("OAuth account mutation ACK", () => {
  it("merges core state and preserves list-only enrichment until refetch", () => {
    const current = parseOAuthAccountConfiguration(accountConfiguration());
    const previous = current.items[0];
    if (!previous) {
      throw new Error("account fixture is missing");
    }
    const raw = accountConfiguration().items[0];
    if (!raw) {
      throw new Error("account fixture is missing");
    }
    const core = Object.fromEntries(
      Object.entries(raw).filter(
        ([key]) => key !== "available_models" && key !== "usage",
      ),
    );
    const incoming = parseOAuthAccountMutationResponse({
      config_revision: 5,
      items: [{
        ...core,
        enabled: false,
        config_version: 5,
        selected_model_count: 1,
        models: ["gpt-5.6-terra"],
      }],
    });

    const merged = mergeOAuthAccountMutationResponse(current, incoming);
    if (!merged) {
      throw new Error("mutation response did not merge");
    }

    expect(merged.items[0]).toMatchObject({
      enabled: false,
      configVersion: 5,
      models: ["gpt-5.6-terra"],
    });
    expect(merged.items[0]?.availableModels).toEqual([
      "codex-auto-review",
      "gpt-5.5",
      "gpt-5.6-terra",
    ]);
    expect(merged.items[0]?.usage).toBe(previous.usage);
    expect(() =>
      parseOAuthAccountMutationResponse({
        config_revision: 5,
        items: [raw],
      }),
    ).toThrow("invalid OAuth account mutation response");
  });
});

function accountConfiguration() {
  return {
    config_revision: 4,
    items: [
      {
        id: "fdcb6e74-820f-4d84-9df6-38af2b031feb",
        provider_kind: "codex",
        label: "Primary Codex",
        requests_per_minute: 2,
        proxy_selection: { mode: "global" },
        enabled: true,
        safe_account_email: "person@example.com",
        expires_at: 1_800_000_000,
        token_version: 2,
        account_generation: 3,
        config_version: 4,
        selected_model_count: 1,
        models: ["gpt-5.5"],
        available_models: ["codex-auto-review", "gpt-5.5"],
        runtime: {
          resolved_proxy: {
            id: "00000000-0000-0000-0000-000000000000",
            name: "DIRECT",
            kind: "direct",
            enabled: true,
          },
          rpm_60s: { used: 1, limit: 2 },
          in_flight: 0,
          status: "ready",
        },
        plan_type: "plus",
        bot_flagged: null,
        token_refresh_failure: null,
        usage: usage(),
      },
    ],
  };
}

function usage() {
  const windowMs = 2 * 60 * 1000;
  const newest = Math.floor(Date.now() / windowMs) * windowMs;
  return {
    total_requests: 3,
    successful_requests: 2,
    failed_requests: 1,
    window_minutes: 2,
    window_slots: Array.from({ length: 30 }, (_, index) => ({
      started_at_ms: newest - (29 - index) * windowMs,
      total_requests: index >= 27 ? 1 : 0,
      successful_requests: index === 27 || index === 29 ? 1 : 0,
      failed_requests: index === 28 ? 1 : 0,
    })),
  };
}
