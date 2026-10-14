import { useEffect } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { subscribeToDocumentVisibilityChange, subscribeToWindowBlur } from "../core/window-focus";

export type FlushOnWindowLeave = () => void | Promise<void>;

/** Flushes pending edits when the settings surface is blurred, hidden, or backgrounded. */
export function useFlushOnWindowLeave(flush: FlushOnWindowLeave): void {
  const flushRef = useLatestRef(flush);

  useEffect(() => {
    if (typeof window === "undefined" || typeof document === "undefined") return;
    const flushNow = () => void flushRef.current();
    const onVisibilityChange = () => {
      if (document.hidden) flushNow();
    };
    const stopBlur = subscribeToWindowBlur(flushNow);
    window.addEventListener("pagehide", flushNow);
    const stopVisibility = subscribeToDocumentVisibilityChange(onVisibilityChange);
    return () => {
      stopBlur();
      window.removeEventListener("pagehide", flushNow);
      stopVisibility();
    };
  }, []);
}
