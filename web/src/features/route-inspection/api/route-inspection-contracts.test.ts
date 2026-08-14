import { expect, test } from "vitest";

import { parseRouteInspection } from "./route-inspection-contracts";

test("parses the finite route inspection contract", () => {
  const parsed = parseRouteInspection(response());

  expect(parsed.configRevision).toBe(9);
  expect(parsed.items.map((item) => item.status)).toEqual([
    "available",
    "no_enabled_candidate",
  ]);
  expect(parsed.items[0]?.operations[0]?.candidateGroups[0]).toEqual({
    providerKind: "codex",
    providerEndpointId: "59857606-6f98-4b86-9e2f-aad423aa65b4",
    providerEndpointName: "Codex Primary",
    upstreamProtocolDialect: "openai_chat_completions",
    enabledCandidateCount: 2,
  });
});

test("rejects status values outside the configuration-state enum", () => {
  const value = response();
  value.items[0]!.status = "rate_limited";
  expect(() => parseRouteInspection(value)).toThrow("invalid route inspection response");
});

function response() {
  return {
    config_revision: 9,
    items: [
      item("available-model", "available", [
        {
          operation: "responses",
          candidate_groups: [
            {
              provider_kind: "codex",
              provider_endpoint_id: "59857606-6f98-4b86-9e2f-aad423aa65b4",
              provider_endpoint_name: "Codex Primary",
              upstream_protocol_dialect: "openai_chat_completions",
              enabled_candidate_count: 2,
            },
          ],
        },
      ]),
      item("disabled-model", "no_enabled_candidate"),
    ],
  };
}

function item(
  publicModel: string,
  status: string,
  operations: unknown[] = [],
) {
  return {
    public_model: publicModel,
    ingress_protocol: "openai_responses",
    published: true,
    status,
    operations,
  };
}
