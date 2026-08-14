import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProviderCredential } from "../api/provider-credential-contracts";
import { ProviderCredentialTableRow } from "./ProviderCredentialTableRow";
import type { ProxyConfiguration } from "@/features/proxies";

test("shows DIRECT without inheriting the OAuth global proxy", () => {
  render(
    <table>
      <tbody>
        <ProviderCredentialTableRow
          credential={credential}
          proxies={proxies}
          pending={false}
          onEdit={vi.fn()}
          onModels={vi.fn()}
          onToggleEnabled={vi.fn()}
          onDelete={vi.fn()}
        />
      </tbody>
    </table>,
  );

  expect(screen.getByText("DIRECT")).toBeInTheDocument();
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
  usage: {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    windowMinutes: 2,
    windowSlots: [],
  },
} satisfies ProviderCredential;

const proxies = {
  configRevision: 2,
  globalProxyId: "f0335fed-e5a9-4081-966b-37efe4a109a8",
  items: [
    {
      id: "00000000-0000-0000-0000-000000000000",
      name: "DIRECT",
      kind: "direct",
      host: null,
      port: null,
      username: null,
      passwordConfigured: false,
      authenticationVersion: 0,
      enabled: true,
      builtIn: true,
      configVersion: 1,
    },
    {
      id: "f0335fed-e5a9-4081-966b-37efe4a109a8",
      name: "OAuth Proxy",
      kind: "http",
      host: "proxy.example.com",
      port: 8080,
      username: null,
      passwordConfigured: false,
      authenticationVersion: 0,
      enabled: true,
      builtIn: false,
      configVersion: 1,
    },
  ],
} satisfies ProxyConfiguration;
