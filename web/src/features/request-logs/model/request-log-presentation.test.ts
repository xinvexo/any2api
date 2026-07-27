import { expect, test } from "vitest";

import { operationLabel, protocolLabel } from "./request-log-presentation";

test("labels OpenAI Images request logs", () => {
  expect(protocolLabel("openai_images")).toBe("Images");
  expect(operationLabel("images_generations")).toBe(
    "/v1/images/generations",
  );
  expect(operationLabel("images_edits")).toBe("/v1/images/edits");
});
