import { renderHook, waitFor } from "@testing-library/react";
import { expect, test } from "vitest";

import {
  listEntrySurfaceAnimationClass,
  useListEntryAnimations,
} from "./useListEntryAnimations";

interface Entry {
  id: string;
  state: string;
}

const getId = (entry: Entry) => entry.id;
const getState = (entry: Entry) => entry.state;

test("maps row-surface animations without reusing full-row effects", () => {
  expect(listEntrySurfaceAnimationClass("arrive")).toBe(
    "log-entry-surface-arrive",
  );
  expect(listEntrySurfaceAnimationClass("complete")).toBe(
    "log-entry-surface-complete",
  );
  expect(listEntrySurfaceAnimationClass(undefined)).toBeUndefined();
});

test("marks prepended logs and processing-to-final transitions", async () => {
  const { result, rerender } = renderHook(
    ({ items }: { items: Entry[] }) =>
      useListEntryAnimations(items, getId, getState, "logs"),
    { initialProps: { items: [{ id: "active", state: "processing" }] } },
  );

  expect(result.current.size).toBe(0);
  rerender({
    items: [
      { id: "new", state: "processing" },
      { id: "active", state: "processing" },
    ],
  });
  await waitFor(() => expect(result.current.get("new")).toBe("arrive"));

  rerender({
    items: [
      { id: "new", state: "processing" },
      { id: "active", state: "success" },
    ],
  });
  await waitFor(() => expect(result.current.get("active")).toBe("complete"));
});

test("does not animate older cursor pages appended to the feed", async () => {
  const { result, rerender } = renderHook(
    ({ items }: { items: Entry[] }) =>
      useListEntryAnimations(items, getId, getState, "logs"),
    { initialProps: { items: [{ id: "latest", state: "success" }] } },
  );

  rerender({
    items: [
      { id: "latest", state: "success" },
      { id: "older", state: "success" },
    ],
  });

  await waitFor(() => expect(result.current.size).toBe(0));
});

test("keeps earlier new-entry effects while later SSE batches arrive", async () => {
  const { result, rerender } = renderHook(
    ({ items }: { items: Entry[] }) =>
      useListEntryAnimations(items, getId, getState, "logs"),
    { initialProps: { items: [{ id: "known", state: "success" }] } },
  );

  rerender({
    items: [
      { id: "new-1", state: "success" },
      { id: "known", state: "success" },
    ],
  });
  await waitFor(() => expect(result.current.get("new-1")).toBe("arrive"));

  rerender({
    items: [
      { id: "new-2", state: "success" },
      { id: "new-1", state: "success" },
      { id: "known", state: "success" },
    ],
  });

  await waitFor(() => {
    expect(result.current.get("new-1")).toBe("arrive");
    expect(result.current.get("new-2")).toBe("arrive");
  });
});
