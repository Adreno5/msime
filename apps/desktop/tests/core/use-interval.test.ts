// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { useInterval } from "../../../../packages/ui/src/core/use-interval";

afterEach(() => {
  vi.useRealTimers();
});

test("runs at the interval and stops after unmount", () => {
  vi.useFakeTimers();
  const callback = vi.fn();
  const { unmount } = renderHook(() => useInterval(callback, 1000));

  act(() => vi.advanceTimersByTime(2500));
  expect(callback).toHaveBeenCalledTimes(2);

  unmount();
  act(() => vi.advanceTimersByTime(2000));
  expect(callback).toHaveBeenCalledTimes(2);
});

test("uses the latest callback without restarting the interval", () => {
  vi.useFakeTimers();
  const first = vi.fn();
  const second = vi.fn();
  const { rerender } = renderHook(({ callback }) => useInterval(callback, 1000), {
    initialProps: { callback: first },
  });

  rerender({ callback: second });
  act(() => vi.advanceTimersByTime(1000));

  expect(first).not.toHaveBeenCalled();
  expect(second).toHaveBeenCalledOnce();
});

test("does not schedule when disabled", () => {
  vi.useFakeTimers();
  const callback = vi.fn();
  renderHook(() => useInterval(callback, null));

  act(() => vi.advanceTimersByTime(3000));
  expect(callback).not.toHaveBeenCalled();
});
