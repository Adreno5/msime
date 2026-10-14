// @vitest-environment jsdom
import { act } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  subscribeToWindowFocus,
  subscribeToWindowResume,
} from "../../../../packages/ui/src/core/window-focus";

afterEach(() => {
  vi.restoreAllMocks();
});

test("subscribes to window focus until its cleanup is called", () => {
  const refresh = vi.fn();
  const stop = subscribeToWindowFocus(refresh);

  act(() => window.dispatchEvent(new Event("focus")));
  expect(refresh).toHaveBeenCalledOnce();

  stop();
  act(() => window.dispatchEvent(new Event("focus")));
  expect(refresh).toHaveBeenCalledOnce();
});

test("resumes on focus and visible transitions, but ignores hidden transitions", () => {
  const refresh = vi.fn();
  const stop = subscribeToWindowResume(refresh);

  act(() => window.dispatchEvent(new Event("focus")));
  expect(refresh).toHaveBeenCalledOnce();

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledOnce();

  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledTimes(2);

  stop();
  act(() => window.dispatchEvent(new Event("focus")));
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(refresh).toHaveBeenCalledTimes(2);
});
