// @vitest-environment jsdom
import { act } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import {
  subscribeToWindowBlur,
  subscribeToWindowFocus,
  subscribeToWindowResume,
} from "../../../../packages/ui/src/core/window-focus";

afterEach(() => {
  vi.restoreAllMocks();
});

test.each([
  ["focus", subscribeToWindowFocus, "blur"],
  ["blur", subscribeToWindowBlur, "focus"],
] as const)("%s 订阅只响应对应事件并独立清理", (event, subscribe, ignoredEvent) => {
  const callback = vi.fn();
  const otherCallback = vi.fn();
  const stop = subscribe(callback);
  const stopOther = subscribe(otherCallback);

  act(() => window.dispatchEvent(new Event(ignoredEvent)));
  expect(callback).not.toHaveBeenCalled();
  expect(otherCallback).not.toHaveBeenCalled();
  act(() => window.dispatchEvent(new Event(event)));
  expect(callback).toHaveBeenCalledOnce();
  expect(otherCallback).toHaveBeenCalledOnce();

  stop();
  act(() => window.dispatchEvent(new Event(event)));
  expect(callback).toHaveBeenCalledOnce();
  expect(otherCallback).toHaveBeenCalledTimes(2);
  stopOther();
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

const blurConsumers = import.meta.glob<string>(
  [
    "../../../../packages/ui/src/keyboard/panels.tsx",
    "../../../../packages/ui/src/keyboard/use-panel-drag.ts",
    "../../../../packages/ui/src/settings/use-flush-on-window-leave.ts",
    "../../../../packages/ui/src/settings/use-settings-window-interactions.ts",
  ],
  { eager: true, query: "?raw", import: "default" },
);

test.each(Object.entries(blurConsumers))("%s 复用公共失焦订阅生命周期", (_path, source) => {
  expect(source).toContain("subscribeToWindowBlur");
  expect(source).not.toContain('window.addEventListener("blur"');
  expect(source).not.toContain('window.removeEventListener("blur"');
});
