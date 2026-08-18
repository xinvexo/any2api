import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import { SystemLogBody } from "./SystemLogBody";
import type { SystemLogBody as Body, SystemLogHeader } from "../api/system-log-contracts";

const JSON_HEADERS: SystemLogHeader[] = [
  { name: "Content-Type", value: "application/problem+json; charset=utf-8", encoding: "utf8" },
];

test("formats and highlights JSON without rewriting exact value lexemes", () => {
  const source =
    '{"id":9007199254740993,"escaped":"\\u4e2d","enabled":true,"missing":null}';
  const view = render(<SystemLogBody body={body(source)} headers={JSON_HEADERS} />);

  const formatted = view.container.querySelector('[data-body-view="formatted-json"]');
  expect(formatted?.querySelector("code")).toHaveAttribute("data-json-highlight", "syntax");
  expect(formatted?.textContent).toBe(
    '{\n  "id": 9007199254740993,\n  "escaped": "\\u4e2d",\n  "enabled": true,\n  "missing": null\n}',
  );
  for (const token of ["key", "string", "number", "boolean", "null"]) {
    expect(view.container.querySelector(`[data-json-token="${token}"]`)).not.toBeNull();
  }

  fireEvent.click(screen.getByRole("button", { name: "查看 JSON 原文" }));
  expect(view.container.querySelector('[data-body-view="raw"]')?.textContent).toBe(source);
  expect(screen.getByRole("button", { name: "格式化 JSON 正文" })).toHaveTextContent("格式化");
});

test("leaves incomplete, binary, untyped, and malformed bodies unchanged", () => {
  const cases: Array<{ body: Body; headers: SystemLogHeader[] }> = [
    { body: body('{"ok":true}', { complete: false }), headers: JSON_HEADERS },
    { body: body('eyJvayI6dHJ1ZX0=', { encoding: "base64" }), headers: JSON_HEADERS },
    { body: body('{"ok":true}'), headers: [] },
    { body: body("not-json"), headers: JSON_HEADERS },
  ];

  for (const item of cases) {
    const view = render(<SystemLogBody body={item.body} headers={item.headers} />);
    expect(view.container.querySelector('[data-body-view="raw"]')).not.toBeNull();
    expect(view.container.querySelector('[data-body-view="formatted-json"]')).toBeNull();
    expect(view.queryByRole("button", { name: "查看 JSON 原文" })).not.toBeInTheDocument();
    view.unmount();
  }
});

test.each([
  {
    name: "formatted character budget",
    source: JSON.stringify({ payload: "x".repeat(256 * 1024) }),
  },
  {
    name: "syntax token budget",
    source: `[${Array(2_050).fill('{"k":1}').join(",")}]`,
  },
])("renders formatted JSON as plain text beyond the $name", ({ source }) => {
  const view = render(<SystemLogBody body={body(source)} headers={JSON_HEADERS} />);

  const formatted = view.container.querySelector('[data-body-view="formatted-json"]');
  expect(formatted?.textContent).toContain("\n");
  expect(formatted?.querySelector("code")).toHaveAttribute("data-json-highlight", "plain");
  expect(formatted?.querySelectorAll("[data-json-token]")).toHaveLength(0);

  fireEvent.click(screen.getByRole("button", { name: "查看 JSON 原文" }));
  expect(view.container.querySelector('[data-body-view="raw"]')?.textContent).toBe(source);
});

function body(
  content: string,
  overrides: Partial<Pick<Body, "encoding" | "complete" | "truncated">> = {},
): Body {
  const capturedBytes = new TextEncoder().encode(content).length;
  const truncated = overrides.truncated ?? false;
  return {
    content,
    encoding: overrides.encoding ?? "utf8",
    capturedBytes,
    totalBytes: truncated ? capturedBytes + 1 : capturedBytes,
    complete: overrides.complete ?? true,
    truncated,
  };
}
