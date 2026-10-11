import { errorCode } from "../core/error-code";
import type { CloudClipboardAction, CloudClipboardResponse } from "../keyboard/panels";

/** One cloud clipboard request, the same actions the 云剪贴板 panel sends, issued by a host for the settings window itself. */
export type CloudClipboardRequest = (
  action: CloudClipboardAction,
) => Promise<CloudClipboardResponse>;

/** The longest text the cloud clipboard stores, counted in UTF-16 code units the way the server counts it. */
export const CLOUD_CLIPBOARD_MAX_UTF16 = 4000;
export const CLOUD_CLIPBOARD_SIGNED_OUT = "登录水杉账号后可在设备间同步剪贴板";
export const CLOUD_CLIPBOARD_DISABLED = "云剪贴板未开启";
export const CLOUD_CLIPBOARD_UNAVAILABLE = "云剪贴板服务暂不可用，请稍后重试";
export const CLOUD_CLIPBOARD_PENDING =
  "本设备还没有获准使用云剪贴板：请打开云剪贴板查看本设备的核对码，再到一台已在使用云剪贴板的设备上点「允许」";

/** Whether this window can send to the cloud clipboard right now: `ready` only when the account is signed in, the server reports the clipboard enabled and this device holds the clipboard key. `pending` means this device is waiting for another one to approve it. */
export type CloudClipboardAvailability =
  | "checking"
  | "ready"
  | "signed_out"
  | "disabled"
  | "pending"
  | "unavailable";

/** Reads a failed request: an unauthorized account means signed out (or a session that has expired), the end-to-end codes name a device still waiting for approval or a clipboard switched off, anything else means the service could not be reached. */
export function cloudClipboardFailure(
  error: unknown,
): "signed_out" | "pending" | "disabled" | "unavailable" {
  const code = errorCode(error);
  if (code === "unauthorized" || code?.endsWith("_unauthorized")) return "signed_out";
  if (code === "clipboard_device_pending") return "pending";
  if (code === "clipboard_sync_disabled") return "disabled";
  return "unavailable";
}

/** 云剪贴板请求失败时给用户看的一句话：驱动给出的错误码各有说明，其余用调用方的 `fallback`。 */
export function cloudClipboardErrorMessage(error: unknown, fallback: string): string {
  switch (errorCode(error)) {
    case "clipboard_device_pending":
      return CLOUD_CLIPBOARD_PENDING;
    case "clipboard_sync_disabled":
      return CLOUD_CLIPBOARD_DISABLED;
    case "clipboard_key_conflict":
      return "另一台设备刚刚更换了云剪贴板密钥，请刷新后重试";
    case "clipboard_device_limit":
      return "这个账号登记的设备已达上限，请先移除不再使用的设备";
    case "not_found":
      return "这台设备已不在列表里，请刷新后再试";
    case "account_unauthorized":
    case "unauthorized":
      return CLOUD_CLIPBOARD_SIGNED_OUT;
  }
  return fallback;
}

/** The note shown for an availability that blocks sending; `undefined` when there is nothing to explain. */
export function cloudClipboardAvailabilityNote(
  availability: CloudClipboardAvailability,
): string | undefined {
  switch (availability) {
    case "signed_out":
      return CLOUD_CLIPBOARD_SIGNED_OUT;
    case "disabled":
      return CLOUD_CLIPBOARD_DISABLED;
    case "pending":
      return CLOUD_CLIPBOARD_PENDING;
    case "unavailable":
      return CLOUD_CLIPBOARD_UNAVAILABLE;
  }
  return undefined;
}
