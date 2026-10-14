export type WindowFocusListener = () => void;

const noop = () => {};

/** 订阅文档可见性变化；显示或隐藏的处理规则由调用方决定。 */
export function subscribeToDocumentVisibilityChange(listener: () => void): () => void {
  if (typeof document === "undefined") return noop;
  document.addEventListener("visibilitychange", listener);
  return () => document.removeEventListener("visibilitychange", listener);
}

function subscribeToWindowEvent(
  event: "focus" | "blur",
  listener: WindowFocusListener,
): () => void {
  if (typeof window === "undefined") return noop;
  window.addEventListener(event, listener);
  return () => window.removeEventListener(event, listener);
}

/** 订阅窗口获得焦点事件，并返回对称的清理函数。 */
export function subscribeToWindowFocus(listener: WindowFocusListener): () => void {
  return subscribeToWindowEvent("focus", listener);
}

/** 订阅窗口失去焦点事件，并返回对称的清理函数。 */
export function subscribeToWindowBlur(listener: WindowFocusListener): () => void {
  return subscribeToWindowEvent("blur", listener);
}

/** 订阅窗口重新获得焦点或从后台回到前台的事件，并返回清理函数。 */
export function subscribeToWindowResume(listener: WindowFocusListener): () => void {
  if (typeof window === "undefined" || typeof document === "undefined") return noop;
  const onVisibilityChange = () => {
    if (document.visibilityState === "visible") listener();
  };
  const stopFocus = subscribeToWindowFocus(listener);
  const stopVisibility = subscribeToDocumentVisibilityChange(onVisibilityChange);
  return () => {
    stopFocus();
    stopVisibility();
  };
}
