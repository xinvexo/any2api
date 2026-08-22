import { expect, test } from "vitest";

import {
  protocolDialectForOperation,
  protocolDialectLabel,
  protocolOperationLabel,
  providerKindLabel,
} from "./provider-protocol-vocabulary";

test("uses one provider and protocol vocabulary across admin features", () => {
  expect(providerKindLabel("openai")).toBe("OpenAI");
  expect(protocolDialectLabel("openai_chat_completions")).toBe(
    "OpenAI Chat Completions",
  );
  expect(protocolOperationLabel("responses_compact")).toBe("响应压缩");
  expect(protocolDialectForOperation("responses_compact")).toBe("openai_responses");
});
