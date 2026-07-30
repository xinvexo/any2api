import { expect, test } from "vitest";

import { resolveMenuLayout } from "./select-model";

test("shrinks a menu to one side of a trigger in a short viewport", () => {
  const menu = { scrollHeight: 168 } as HTMLDivElement;
  const trigger = new DOMRect(20, 150, 200, 32);

  const layout = resolveMenuLayout(trigger, menu, 5, {
    innerWidth: 390,
    innerHeight: 320,
  });

  expect(layout.maxHeight).toBe(138);
  expect(layout.top + layout.maxHeight).toBeLessThanOrEqual(trigger.top - 4);
});

test("opens below without shrinking when the requested menu fits", () => {
  const menu = { scrollHeight: 104 } as HTMLDivElement;
  const trigger = new DOMRect(20, 40, 200, 32);

  const layout = resolveMenuLayout(trigger, menu, 3, {
    innerWidth: 390,
    innerHeight: 320,
  });

  expect(layout.top).toBe(trigger.bottom + 4);
  expect(layout.maxHeight).toBe(104);
});
