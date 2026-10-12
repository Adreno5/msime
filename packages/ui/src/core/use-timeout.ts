import { useEffect } from "react";

/** Runs a callback after a delay and cancels it when the owner unmounts or changes. */
export function useTimeout(callback: () => void, delayMs: number): void {
  useEffect(() => {
    const timer = window.setTimeout(callback, delayMs);
    return () => window.clearTimeout(timer);
  }, [callback, delayMs]);
}
