import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

vi.mock("@/app/providers", () => ({
  AppProviders: () => {
    throw new Error("provider failed");
  },
}));
vi.mock("@/app/router", () => ({ router: {} }));

import { App } from "@/app/App";

test("the App boundary sits outside providers", () => {
  vi.spyOn(console, "error").mockImplementation(() => undefined);

  render(<App />);

  expect(screen.getByRole("heading", { name: "管理界面发生错误" })).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "重新加载" })).toBeInTheDocument();
});
