// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LocalModesSection, type LocalModePreferences } from "@msime/ui";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const preferences: LocalModePreferences = {
  unicode: true,
  date_time: true,
  quick_phrase: true,
  emoji: true,
  kaomoji: true,
  super_jianpin: true,
  temporary_english: true,
  temporary_japanese: true,
};

test("local mode section updates only the selected mode", () => {
  const onChange = vi.fn();
  render(<LocalModesSection preferences={preferences} ios={false} onChange={onChange} />);

  fireEvent.click(screen.getByRole("switch", { name: /^Unicode/ }));

  expect(onChange).toHaveBeenCalledWith({ ...preferences, unicode: false });
});

test("iOS local mode descriptions use the touch keyboard entry path", () => {
  render(<LocalModesSection preferences={preferences} ios onChange={vi.fn()} />);

  expect(screen.getByText(/「更多 → 本地输入」里选「快捷短语」/)).toBeTruthy();
  expect(screen.queryByText(/Shift\+K/)).toBeNull();
});

test("the Shift+letter master switch is shown only where the host asks for it", () => {
  render(<LocalModesSection preferences={preferences} ios={false} onChange={vi.fn()} />);
  expect(screen.queryByRole("switch", { name: /Shift\+字母进入快捷模式/ })).toBeNull();
});

test("turning the Shift+letter master switch off keeps every mode switch as it was", () => {
  const onChange = vi.fn();
  const { rerender } = render(
    <LocalModesSection
      preferences={preferences}
      ios={false}
      triggers
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  const master = screen.getByRole("switch", { name: /Shift\+字母进入快捷模式/ });
  // 早于总开关的文档没有这个键，读作开。
  expect((master as HTMLInputElement).checked).toBe(true);
  fireEvent.click(master);
  expect(onChange).toHaveBeenCalledWith({ ...preferences, shift_letter_entry: false });

  rerender(
    <LocalModesSection
      preferences={{ ...preferences, shift_letter_entry: false }}
      ios={false}
      triggers
      shiftLetterEntrySwitch
      onChange={onChange}
    />,
  );
  // Shift+字母的模式暂时不起作用，开关保留原值但不能改；/ 不经过 Shift，照常可改。
  const unicode = screen.getByRole("switch", { name: /^Unicode/ });
  expect((unicode as HTMLInputElement).checked).toBe(true);
  expect((unicode as HTMLInputElement).disabled).toBe(true);
  const expression = screen.getByRole("switch", { name: /V 模式/ });
  expect((expression as HTMLInputElement).disabled).toBe(true);
  const command = screen.getByRole("switch", { name: /\/ 模式/ });
  expect((command as HTMLInputElement).disabled).toBe(false);
});
