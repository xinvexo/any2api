import { expect, test } from "vitest";

import {
  formatResourceBytes,
  formatResourcePercent,
  formatSystemMemory,
} from "./overview-resources-presentation";

test("formats binary memory units and CPU percentages", () => {
  expect(formatResourceBytes(1023)).toBe("1023 B");
  expect(formatResourceBytes(1024)).toBe("1 KiB");
  expect(formatResourceBytes(1024 ** 2)).toBe("1 MiB");
  expect(formatResourcePercent(12.34)).toBe("12.3%");
});

test("presents system memory as usage percentage with an absolute note", () => {
  expect(formatSystemMemory(8 * 1024 ** 3, 16 * 1024 ** 3)).toEqual({
    value: "50%",
    note: "8 GiB / 16 GiB",
  });
});
