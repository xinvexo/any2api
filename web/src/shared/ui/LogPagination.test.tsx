import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { LogPagination } from "./LogPagination";

test("jumps to an exact page and exposes first and last page controls", () => {
  const onPageChange = vi.fn();

  render(
    <LogPagination
      page={4}
      pageSize={20}
      total={200}
      hasNextPage
      onPageChange={onPageChange}
      onPageSizeChange={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByRole("spinbutton", { name: "页码，共 10 页" }), {
    target: { value: "8" },
  });
  fireEvent.submit(screen.getByRole("spinbutton").closest("form")!);
  fireEvent.click(screen.getByRole("button", { name: "首页" }));
  fireEvent.click(screen.getByRole("button", { name: "末页" }));

  expect(onPageChange.mock.calls).toEqual([[8], [1], [10]]);
  expect(screen.queryByRole("button", { name: "跳转到页码" })).not.toBeInTheDocument();
});

test("clamps an out-of-range page and disables terminal navigation", () => {
  const onPageChange = vi.fn();
  const { rerender } = render(
    <LogPagination
      page={4}
      pageSize={20}
      total={200}
      hasNextPage
      onPageChange={onPageChange}
      onPageSizeChange={vi.fn()}
    />,
  );

  fireEvent.change(screen.getByRole("spinbutton"), { target: { value: "999" } });
  fireEvent.submit(screen.getByRole("spinbutton").closest("form")!);
  expect(onPageChange).toHaveBeenCalledWith(10);

  rerender(
    <LogPagination
      page={10}
      pageSize={20}
      total={200}
      hasNextPage={false}
      onPageChange={onPageChange}
      onPageSizeChange={vi.fn()}
    />,
  );

  expect(screen.getByRole("spinbutton")).toHaveValue(10);
  expect(screen.getByRole("button", { name: "下一页" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "末页" })).toBeDisabled();
});
