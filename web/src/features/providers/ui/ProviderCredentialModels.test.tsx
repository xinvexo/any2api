import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ProviderCredential,
  ProviderCredentialTestResult,
} from "../api/provider-credential-contracts";
import { ProviderCredentialModels } from "./ProviderCredentialModels";

test("does not misdiagnose an authentication response as a rejected API Key", () => {
  renderModels({
    reachable: true,
    accepted: false,
    catalogValid: false,
    statusCode: 401,
  });

  expect(screen.getByRole("alert")).toHaveTextContent(
    "模型目录请求被上游拒绝（HTTP 401）；请核对 Base URL 与上游认证要求，也可手动添加模型。",
  );
  expect(screen.getByLabelText("手动添加模型")).toBeEnabled();
  expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
});

test("keeps a saved model visible when the refreshed catalog no longer returns it", () => {
  renderModels(
    {
      models: ["gpt-new"],
    },
    {
      models: [{ upstreamModel: "gpt-old", publicModel: null }],
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
  expect(onSave).toHaveBeenCalledWith([
    { upstreamModel: "gpt-5.6-sol", publicModel: null },
  ]);
});

test("keeps manual model editing available while discovery is pending", () => {
  const { onSave } = renderModels(null, {}, { discovering: true });

  expect(screen.getByText("正在读取上游模型")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("手动添加模型"), {
    target: { value: "claude-manual" },
  });
  fireEvent.keyDown(screen.getByLabelText("手动添加模型"), { key: "Enter" });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(onSave).toHaveBeenCalledWith([
    { upstreamModel: "claude-manual", publicModel: null },
  ]);
});

test("edits a public alias on a selected model and saves it as the entry alias", () => {
  const { onSave } = renderModels(
    { models: ["gpt-5.6-sol-ganen"] },
    {
      models: [{ upstreamModel: "gpt-5.6-sol-ganen", publicModel: "gpt-5.6-sol" }],
    },
  );

  const aliasInput = screen.getByLabelText("gpt-5.6-sol-ganen 的公开名称");
  expect(aliasInput).toHaveValue("gpt-5.6-sol");

  fireEvent.change(aliasInput, { target: { value: "gpt-5.6-sol-renamed" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));

  expect(onSave).toHaveBeenCalledWith([
    { upstreamModel: "gpt-5.6-sol-ganen", publicModel: "gpt-5.6-sol-renamed" },
  ]);
});

test("blocks saving when two selections resolve to the same public name", () => {
  const { onSave } = renderModels({ models: [] });

  for (const model of ["upstream-a", "upstream-b"]) {
    fireEvent.change(screen.getByLabelText("手动添加模型"), {
      target: { value: model },
    });
    fireEvent.click(screen.getByRole("button", { name: "添加" }));
  }
  fireEvent.change(screen.getByLabelText("upstream-b 的公开名称"), {
    target: { value: "upstream-a" },
  });

  expect(
    screen.getByText(
      "公开名称「upstream-a」同时来自「upstream-a」和「upstream-b」，请修改其中一个",
    ),
  ).toBeInTheDocument();
  const save = screen.getByRole("button", { name: "保存" });
  expect(save).toBeDisabled();
  fireEvent.click(save);
  expect(onSave).not.toHaveBeenCalled();

  fireEvent.change(screen.getByLabelText("upstream-b 的公开名称"), {
    target: { value: "public-b" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSave).toHaveBeenCalledWith([
    { upstreamModel: "upstream-a", publicModel: null },
    { upstreamModel: "upstream-b", publicModel: "public-b" },
  ]);
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
