// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { useAccountVerification } from "../../../../packages/ui/src/account/use-account-verification";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-01-01T00:00:00Z"));
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

test("verification requests trim their target and share resend and expiry timing", async () => {
  const client = {
    requestCode: vi.fn().mockResolvedValue({ challengeId: "synthetic-challenge", expiresIn: 300 }),
  };
  const setNotice = vi.fn();
  const { result, unmount } = renderHook(() =>
    useAccountVerification({
      client,
      mounted: { current: true },
      clientGeneration: { current: 1 },
      setNotice,
    }),
  );

  act(() => result.current.selectChannel("email"));
  act(() => result.current.updateTarget("  synthetic@example.test  "));
  await act(async () => result.current.sendCode());
  expect(client.requestCode).toHaveBeenCalledWith("email", "synthetic@example.test");
  expect(result.current.challenge?.challengeId).toBe("synthetic-challenge");
  expect(result.current.resendSeconds).toBe(60);
  expect(result.current.expired).toBe(false);
  expect(setNotice).toHaveBeenCalledWith("验证码已发送，请查收。");

  act(() => vi.advanceTimersByTime(60_000));
  expect(result.current.resendSeconds).toBe(0);
  act(() => vi.advanceTimersByTime(240_000));
  expect(result.current.expired).toBe(true);
  act(() => result.current.updateTarget("next@example.test"));
  expect(result.current.challenge).toBeNull();
  unmount();
  expect(vi.getTimerCount()).toBe(0);
});

test("a request from an old client generation cannot publish a challenge", async () => {
  let resolve!: (value: { challengeId: string; expiresIn: number }) => void;
  const client = {
    requestCode: vi.fn(
      () => new Promise<{ challengeId: string; expiresIn: number }>((done) => (resolve = done)),
    ),
  };
  const value = { challengeId: "stale-challenge", expiresIn: 300 };
  const clientGeneration = { current: 1 };
  const setNotice = vi.fn();
  const { result } = renderHook(() =>
    useAccountVerification({ client, mounted: { current: true }, clientGeneration, setNotice }),
  );
  act(() => result.current.selectChannel("phone"));
  act(() => result.current.updateTarget("+810000000000"));
  let pending!: Promise<void>;
  act(() => {
    pending = result.current.sendCode();
  });
  clientGeneration.current = 2;
  await act(async () => {
    resolve(value);
    await pending;
  });
  expect(result.current.challenge).toBeNull();
  expect(setNotice).not.toHaveBeenCalled();
});
