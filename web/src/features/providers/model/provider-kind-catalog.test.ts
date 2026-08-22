import { expect, test } from "vitest";

import { PROVIDER_KIND_OPTIONS } from "./provider-kind-catalog";

test("keeps standard OpenAI after the provider-specific categories", () => {
  expect(PROVIDER_KIND_OPTIONS.map((option) => option.kind)).toEqual([
    "codex",
    "claude",
    "grok",
    "kimi",
    "openai",
  ]);
});
