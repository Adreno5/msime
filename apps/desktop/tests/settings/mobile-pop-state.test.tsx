// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import { useMobilePopState } from "@msime/ui";

function Probe({
  mobile,
  onPopState,
}: {
  mobile: boolean;
  onPopState: (event: PopStateEvent) => void;
}) {
  useMobilePopState(mobile, onPopState);
  return null;
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

test("registers mobile popstate and uses the latest callback", () => {
  const first = vi.fn();
  const current = vi.fn();
  const { rerender } = render(<Probe mobile onPopState={first} />);
  rerender(<Probe mobile onPopState={current} />);

  const event = new PopStateEvent("popstate", { state: { page: "account" } });
  window.dispatchEvent(event);

  expect(first).not.toHaveBeenCalled();
  expect(current).toHaveBeenCalledWith(event);
});

test("does not listen on desktop and removes the listener on unmount", () => {
  const onPopState = vi.fn();
  const { rerender, unmount } = render(<Probe mobile={false} onPopState={onPopState} />);
  window.dispatchEvent(new PopStateEvent("popstate"));
  expect(onPopState).not.toHaveBeenCalled();

  rerender(<Probe mobile onPopState={onPopState} />);
  unmount();
  window.dispatchEvent(new PopStateEvent("popstate"));
  expect(onPopState).not.toHaveBeenCalled();
});

test("settings navigation reuses the shared mobile popstate lifecycle", () => {
  const source = Object.values(
    import.meta.glob<string>("../../../../packages/ui/src/settings/use-settings-navigation.ts", {
      eager: true,
      query: "?raw",
      import: "default",
    }),
  )[0];

  expect(source).toContain('from "./use-mobile-pop-state"');
  expect(source).toContain("useMobilePopState");
  expect(source).not.toContain('window.addEventListener("popstate"');
});
