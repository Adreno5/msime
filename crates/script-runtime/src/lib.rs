//! 桌面脚本插件的沙箱运行时。
//!
//! 只在桌面编译（macOS、Windows、非 ohos 的 Linux），移动端和 wasm32 上整个 crate 为空。

#![cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]

// 模块缓存是进程级的，由后台编译线程填充、各会话线程读取，所以 `Module` 必须能跨线程共享。wasmi 2.0.0 的 `Module` 是 `Arc<ModuleInner>`，这条断言在编译期确认它确实是 Send + Sync；不成立时退回在会话线程的 focus 路径上编译。
const _: fn() = || {
    fn shared_across_threads<T: Send + Sync>() {}
    shared_across_threads::<wasmi::Module>();
};
