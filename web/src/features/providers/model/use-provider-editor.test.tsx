import { act, renderHook } from "@testing-library/react";
import { expect, test } from "vitest";

import type { ProviderProtocolOptions } from "../api/provider-contracts";
import { useProviderEditor } from "./use-provider-editor";

const KIMI_OPTIONS = [
  {
    providerKind: "kimi",
    acceptedProtocol: "openai_responses",
    upstreamProtocols: ["openai_chat_completions"],
  },
  {
    providerKind: "kimi",
    acceptedProtocol: "openai_chat_completions",
    upstreamProtocols: ["openai_chat_completions"],
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
