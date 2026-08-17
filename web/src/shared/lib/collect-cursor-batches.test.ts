import { expect, test, vi } from "vitest";

import { collectCursorBatches } from "./collect-cursor-batches";

test("follows cursors until a batch overlaps known data", async () => {
  const fetchBatch = vi.fn(async (cursor: string | null) => {
    if (cursor === null) return batch(["new-1", "new-2"], "cursor-2");
    if (cursor === "cursor-2") return batch(["new-3", "new-4"], "cursor-3");
    return batch(["new-5", "known"], "cursor-4");
  });

  const result = await collectCursorBatches<string, StringBatch>(
    fetchBatch,
    new Set(["known"]),
    (item) => item,
    10,
  );

  expect(result.pageParams).toEqual([null, "cursor-2", "cursor-3"]);
  expect(result.pages.flatMap((page) => page.items)).toEqual([
    "new-1", "new-2", "new-3", "new-4", "new-5", "known",
  ]);
  expect(fetchBatch).toHaveBeenCalledTimes(3);
});

test("bounds recovery when no known row remains in retention", async () => {
  const fetchBatch = vi.fn(async (cursor: string | null) =>
    batch([cursor ?? "latest"], `after-${cursor ?? "latest"}`),
  );

  const result = await collectCursorBatches<string, StringBatch>(
    fetchBatch,
    new Set(["missing"]),
    (item) => item,
    3,
  );

  expect(result.pages).toHaveLength(3);
  expect(fetchBatch).toHaveBeenCalledTimes(3);
});

test("stops if a server repeats the same cursor", async () => {
  const fetchBatch = vi.fn(async (cursor: string | null) =>
    batch([cursor ?? "latest"], "repeated"),
  );

  const result = await collectCursorBatches<string, StringBatch>(
    fetchBatch,
    new Set(["missing"]),
    (item) => item,
    10,
  );

  expect(result.pageParams).toEqual([null, "repeated"]);
  expect(fetchBatch).toHaveBeenCalledTimes(2);
});

function batch(items: string[], nextCursor: string | null) {
  return { items, nextCursor };
}

type StringBatch = ReturnType<typeof batch>;
