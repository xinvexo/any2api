import { act, renderHook } from "@testing-library/react";
import { expect, test } from "vitest";

import type { ProviderProtocolOptions } from "../api/provider-contracts";
import { useProviderEditor } from "./use-provider-editor";

const OPENAI_OPTIONS = [
  {
    providerKind: "openai",
    acceptedProtocol: "openai_responses",
    upstreamOptions: [
      {
        protocol: "openai_responses",
        fidelity: "direct",
        operations: ["responses", "responses_compact"],
        bridge: null,
      },
    ],
  },
] satisfies ProviderProtocolOptions[];

const KIMI_OPTIONS = [
  {
    providerKind: "kimi",
    acceptedProtocol: "openai_responses",
    upstreamOptions: [translatedOption("openai_chat_completions", ["responses"])],
  },
  {
    providerKind: "kimi",
    acceptedProtocol: "openai_chat_completions",
    upstreamOptions: [directOption("openai_chat_completions", ["chat_completions"])],
  },
] satisfies ProviderProtocolOptions[];

test("defaults Kimi Responses endpoints to the required Chat bridge", () => {
  const { result } = renderHook(() =>
    useProviderEditor(undefined, "kimi", KIMI_OPTIONS),
  );

  expect(result.current.draft).toMatchObject({
    providerKind: "kimi",
    baseUrl: "https://api.moonshot.cn/v1",
    protocolDialect: "openai_responses",
    upstreamProtocolDialect: "openai_chat_completions",
  });

  act(() => result.current.updateProtocolDialect("openai_chat_completions"));
  expect(result.current.draft.upstreamProtocolDialect).toBeNull();

  act(() => result.current.updateProtocolDialect("openai_responses"));
  expect(result.current.draft.upstreamProtocolDialect).toBe(
    "openai_chat_completions",
  );
});

test("defaults standard OpenAI to the official Responses endpoint", () => {
  const { result } = renderHook(() =>
    useProviderEditor(undefined, "openai", OPENAI_OPTIONS),
  );

  expect(result.current.draft).toMatchObject({
    providerKind: "openai",
    baseUrl: "https://api.openai.com/v1",
    protocolDialect: "openai_responses",
    upstreamProtocolDialect: null,
  });
});

function directOption(
  protocol: "openai_chat_completions",
  operations: ["chat_completions"],
) {
  return { protocol, fidelity: "direct" as const, operations, bridge: null };
}

function translatedOption(
  protocol: "openai_chat_completions",
  operations: ["responses"],
) {
  return {
    protocol,
    fidelity: "translated" as const,
    operations,
    bridge: {
      contractId: "openai-responses-to-chat-completions/v2",
      requestFields: [{ path: "input", behavior: "translated" as const }],
      toolTypes: ["function"],
      limitations: [
        {
          code: "canonical_request_reconstruction",
          description: "The request is reconstructed.",
        },
      ],
    },
  };
}
