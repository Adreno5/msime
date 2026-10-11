#pragma once

#include <string_view>

namespace msime::linux_host {

// 单按 Caps Lock 切换中英文之后要把大写锁定撤回，宿主只在 X11 会话里做得到：XKB 的锁定状态归 X 服务器管，任何客户端都能改。Wayland 会话里锁定状态归合成器，XWayland 的 XKB 改了也只影响 X 客户端，所以那里一律不撤回，轻点照常是大写锁定。
//
// 参数是会话的环境变量（WAYLAND_DISPLAY、XDG_SESSION_TYPE、DISPLAY），空指针当作没设，测试直接传值。
inline bool caps_lock_release_supported(const char *wayland_display, const char *session_type,
                                        const char *display) {
  if (wayland_display && *wayland_display) return false;
  if (session_type && std::string_view(session_type) == "wayland") return false;
  return display && *display;
}

// 用当前进程的环境判断：宿主编进了 X11 支持，并且是 X11 会话。
bool caps_lock_release_available();

// 把 X11 会话的大写锁定撤回关闭，成功返回 true。不可用或失败时什么都不改。
bool release_caps_lock();

} // namespace msime::linux_host
