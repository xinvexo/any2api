import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { ScrollToTopButton } from "./ScrollToTopButton";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("uses the document scroll threshold on mobile", () => {
  let scrollY = 0;
  const scrollTo = vi.fn();
  vi.stubGlobal("matchMedia", createMatchMedia(true));
  vi.spyOn(window, "scrollY", "get").mockImplementation(() => scrollY);
  vi.stubGlobal("scrollTo", scrollTo);
  const onClick = vi.fn();

  render(<ScrollToTopButton visible onClick={onClick} />);
  expect(screen.queryByRole("button", { name: "回到顶部" })).not.toBeInTheDocument();

  scrollY = 320;
  fireEvent.scroll(window);
  fireEvent.click(screen.getByRole("button", { name: "回到顶部" }));

  expect(scrollTo).toHaveBeenCalledWith({ top: 0, behavior: "smooth" });
  expect(onClick).toHaveBeenCalledTimes(1);
});

test("keeps the caller-controlled visibility on desktop", () => {
  vi.stubGlobal("matchMedia", createMatchMedia(false));
  const onClick = vi.fn();
  const rendered = render(<ScrollToTopButton visible={false} onClick={onClick} />);

  expect(screen.queryByRole("button", { name: "回到顶部" })).not.toBeInTheDocument();
  rendered.rerender(<ScrollToTopButton visible onClick={onClick} />);
  fireEvent.click(screen.getByRole("button", { name: "回到顶部" }));

  expect(onClick).toHaveBeenCalledTimes(1);
});

function createMatchMedia(matches: boolean) {
  return vi.fn().mockReturnValue({
    matches,
    media: "(max-width: 767px)",
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  });
}
