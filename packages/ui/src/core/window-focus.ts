export type WindowFocusListener = () => void;

const noop = () => {};

/** 订阅窗口获得焦点事件，并返回对称的清理函数。 */
export function subscribeToWindowFocus(listener: WindowFocusListener): () => void {
  if (typeof window === "undefined") return noop;
  window.addEventListener("focus", listener);
  return () => window.removeEventListener("focus", listener);
}

/** 订阅窗口重新获得焦点或从后台回到前台的事件，并返回清理函数。 */
export function subscribeToWindowResume(listener: WindowFocusListener): () => void {
  if (typeof window === "undefined" || typeof document === "undefined") return noop;
  const onVisibilityChange = () => {
    if (document.visibilityState === "visible") listener();
  };
  const stopFocus = subscribeToWindowFocus(listener);
  document.addEventListener("visibilitychange", onVisibilityChange);
  return () => {
    stopFocus();
    document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}
