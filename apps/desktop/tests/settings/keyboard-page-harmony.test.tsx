// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { SettingsPage, type Snapshot } from "@msime/ui";
import { testHost } from "../support/host";
import { settingsFormReady } from "../support/settings-form";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState({}, "");
});

const snapshot = {
  format_version: 1,
  revision: 3,
  preferences: { scheme: "quanpin", candidate_page_size: 5, learning: true },
} as Snapshot;

function renderKeyboardPage(shortcuts: boolean) {
  render(
    <SettingsPage
      initialPage="screen-keyboard"
      client={{
        load: vi.fn().mockResolvedValue(snapshot),
        save: vi.fn(),
        host: testHost({
          platform: "harmony",
          mobile_settings: true,
          panel_windows: false,
          mode_switch_shortcuts: shortcuts,
        }),
      }}
    />,
  );
}

test("the phone 键盘 page opens 外接键盘快捷键 from its 外接键盘 group", async () => {
  renderKeyboardPage(true);
  await settingsFormReady();
  const external = screen.getByRole("region", { name: "外接键盘" });
  fireEvent.click(within(external).getByRole("button", { name: "外接键盘快捷键" }));
  // 打开的是快捷键页：它的第一行是中英切换。
  expect(await screen.findByText("中/英文切换")).toBeTruthy();
});

test("a host that routes no keyboard shortcuts has no 外接键盘 group on the 键盘 page", async () => {
  renderKeyboardPage(false);
  await settingsFormReady();
  expect(screen.queryByRole("region", { name: "外接键盘" })).toBeNull();
});
