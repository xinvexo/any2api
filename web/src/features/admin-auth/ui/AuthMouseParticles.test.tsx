import { fireEvent, render } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import { AuthMouseParticles } from "./AuthMouseParticles";

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

test("does not run animation frames until pointer movement emits particles", () => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue({
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    beginPath: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    fillStyle: "",
  } as unknown as CanvasRenderingContext2D);
  vi.spyOn(performance, "now").mockReturnValue(100);
  const requestFrame = vi.fn(() => 1);
  vi.stubGlobal("requestAnimationFrame", requestFrame);
  vi.stubGlobal("cancelAnimationFrame", vi.fn());

  const view = render(
    <div>
      <AuthMouseParticles />
    </div>,
  );
  expect(requestFrame).not.toHaveBeenCalled();

  const canvas = view.container.querySelector("canvas");
  expect(canvas).not.toBeNull();
  fireEvent.pointerMove(canvas!.parentElement!, { clientX: 20, clientY: 20 });
  expect(requestFrame).toHaveBeenCalledTimes(1);
});
