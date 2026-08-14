import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import { ProviderCredentialTableRow } from "./ProviderCredentialTableRow";

test("shows DIRECT without inheriting the OAuth global proxy", () => {
  render(
    <table>
      <tbody>
        <ProviderCredentialTableRow
          credential={credential}
          pending={false}
          onEdit={vi.fn()}
          onModels={vi.fn()}
          onToggleEnabled={vi.fn()}
          onDelete={vi.fn()}
        />
      </tbody>
    </table>,
  );

  expect(screen.getByText("DIRECT（直连）")).toBeInTheDocument();
  expect(screen.getByText("无限制（未计窗口）")).toBeInTheDocument();
  expect(screen.getByText("正常")).toBeInTheDocument();
  expect(screen.queryByText(/继承/)).not.toBeInTheDocument();
  expect(screen.queryByText("OAuth Proxy")).not.toBeInTheDocument();
});

const credential = {
  id: "75072ca7-d922-428d-a4f8-86401567da32",
  providerEndpointId: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
  label: "Primary Key",
  credentialKind: "api_key",
  fingerprint: "v2:0123456789abcdef",
  secretTail: "test",
  proxyProfileId: "00000000-0000-0000-0000-000000000000",
  requestsPerMinute: null,
  enabled: true,
  secretVersion: 1,
  credentialGeneration: 1,
  configVersion: 1,
  models: [],
  runtime: {
    resolvedProxy: {
      id: "00000000-0000-0000-0000-000000000000",
      name: "DIRECT",
      kind: "direct",
      enabled: true,
    },
    rpm60s: { used: 0, limit: null },
    inFlight: 0,
    status: "ready",
  },
  usage: {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    windowMinutes: 2,
    windowSlots: [],
  },
} satisfies ProviderCredential;
