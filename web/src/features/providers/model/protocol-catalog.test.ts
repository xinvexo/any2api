import { expect, test } from "vitest";

import { protocolLabel } from "./protocol-catalog";

test("labels every supported protocol", () => {
  expect(protocolLabel("openai_responses")).toBe("OpenAI Responses");
  expect(protocolLabel("openai_chat_completions")).toBe("OpenAI Chat Completions");
  expect(protocolLabel("openai_images")).toBe("OpenAI Images");
  expect(protocolLabel("anthropic_messages")).toBe("Anthropic Messages");
});
