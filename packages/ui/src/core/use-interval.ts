import { useEffect } from "react";
import { useLatestRef } from "./use-latest-ref";

/** 按间隔运行回调；间隔为空时停用，并在组件卸载或间隔变化时清理定时器。 */
export function useInterval(callback: () => void, delayMs: number | null): void {
  const callbackRef = useLatestRef(callback);
  useEffect(() => {
    if (delayMs === null) return;
    const timer = window.setInterval(() => callbackRef.current(), delayMs);
    return () => window.clearInterval(timer);
  }, [delayMs]);
}
