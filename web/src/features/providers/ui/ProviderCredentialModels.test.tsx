import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import { ProviderCredentialModels } from "./ProviderCredentialModels";

test("shows an explicit message when the upstream rejects the API Key", () => {
  renderModels({
    reachable: true,
    accepted: false,
    catalogValid: false,
    statusCode: 401,
  });

  expect(screen.getByRole("alert")).toHaveTextContent("上游拒绝了这把 API Key（HTTP 401）");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

test("keeps a saved model visible when the refreshed catalog no longer returns it", () => {
  renderModels(
    {
      models: ["gpt-new"],
    },
    {
      models: ["gpt-old"],
    },
  );

  expect(screen.getByRole("checkbox", { name: "gpt-new" })).not.toBeChecked();
  expect(screen.getByRole("checkbox", { name: "gpt-old" })).toBeChecked();
  expect(screen.getByText("已保存")).toBeInTheDocument();
});

function renderModels(
  resultOverrides: Partial<ProviderCredentialTestResult>,
  credentialOverrides: Partial<ProviderCredential> = {},
) {
  return render(
    <ProviderCredentialModels
      credential={{ ...credential, ...credentialOverrides }}
      result={{ ...acceptedResult, ...resultOverrides }}
      pending={false}
      error={null}
      onDiscover={vi.fn()}
      onSave={vi.fn(async () => undefined)}
      onClose={vi.fn()}
    />,
  );
}

const credential: ProviderCredential = {
  id: "75072ca7-d922-428d-a4f8-86401567da32",
  providerEndpointId: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
  label: "Primary Key",
  credentialKind: "api_key",
  fingerprint: "v1:0123456789abcdef",
  secretTail: "test",
  proxyProfileId: "00000000-0000-0000-0000-000000000000",
  maxConcurrency: 4,
  enabled: true,
  secretSchemaVersion: 1,
  secretVersion: 1,
  credentialGeneration: 1,
  configVersion: 1,
  models: [],
  usage: {
    totalRequests: 0,
    successfulRequests: 0,
    failedRequests: 0,
    windowMinutes: 2,
    windowSlots: Array.from({ length: 30 }, (_, index) => ({
      startedAtMs: 1_900_000_000_000 + index * 120_000,
      totalRequests: 0,
      successfulRequests: 0,
      failedRequests: 0,
    })),
  },
};

const acceptedResult: ProviderCredentialTestResult = {
  configRevision: 3,
  providerEndpointConfigVersion: 1,
  credentialConfigVersion: 1,
  credentialGeneration: 1,
  secretVersion: 1,
  proxyConfigVersion: 1,
  credentialId: credential.id,
  providerEndpointId: credential.providerEndpointId,
  proxyId: credential.proxyProfileId,
  reachable: true,
  accepted: true,
  catalogValid: true,
  statusCode: 200,
  latencyMs: 18,
  authErrorCleared: true,
  errorStage: null,
  failureScope: null,
  models: [],
};
