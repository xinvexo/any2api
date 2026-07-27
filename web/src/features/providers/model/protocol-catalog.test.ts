import { expect, test } from "vitest";

import { protocolLabel } from "./protocol-catalog";

test("labels the OpenAI Images protocol", () => {
  expect(protocolLabel("openai_images")).toBe("OpenAI Images");
});
