import { fireEvent, render, screen } from "@testing-library/react";
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
  expect(screen.getByLabelText("手动添加模型")).toBeEnabled();
  expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
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

test("adds and saves an exact model name when the upstream catalog is empty", () => {
  const { onSave } = renderModels({ models: [] });

  fireEvent.change(screen.getByLabelText("手动添加模型"), {
    target: { value: "gpt-5.6-sol" },
  });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));

  expect(screen.getByRole("checkbox", { name: "gpt-5.6-sol" })).toBeChecked();
  expect(screen.getByText("手动")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSave).toHaveBeenCalledWith(["gpt-5.6-sol"]);
});

test("keeps manual model editing available while discovery is pending", () => {
  const { onSave } = renderModels(null, {}, { discovering: true });

  expect(screen.getByText("正在读取上游模型")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("手动添加模型"), {
    target: { value: "claude-manual" },
  });
  fireEvent.keyDown(screen.getByLabelText("手动添加模型"), { key: "Enter" });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(onSave).toHaveBeenCalledWith(["claude-manual"]);
});

function renderModels(
  resultOverrides: Partial<ProviderCredentialTestResult> | null,
  credentialOverrides: Partial<ProviderCredential> = {},
  stateOverrides: { discovering?: boolean; saving?: boolean } = {},
) {
  const onSave = vi.fn(async () => undefined);
  const view = render(
    <ProviderCredentialModels
      credential={{ ...credential, ...credentialOverrides }}
      result={resultOverrides ? { ...acceptedResult, ...resultOverrides } : undefined}
      discovering={stateOverrides.discovering ?? false}
      saving={stateOverrides.saving ?? false}
      error={null}
      onDiscover={vi.fn()}
      onSave={onSave}
      onClose={vi.fn()}
    />,
  );
  return { ...view, onSave };
}

const credential: ProviderCredential = {
  id: "75072ca7-d922-428d-a4f8-86401567da32",
  providerEndpointId: "1e96eff2-7b3f-4974-b013-8fd2f44c8c1f",
  label: "Primary Key",
  credentialKind: "api_key",
  fingerprint: "v1:0123456789abcdef",
  secretTail: "test",
  proxyProfileId: "00000000-0000-0000-0000-000000000000",
  requestsPerMinute: null,
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
