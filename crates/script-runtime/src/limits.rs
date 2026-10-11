//! 资源限制的初值（设计 §4.4），第 3 阶段在真机上标定后再调。
//!
//! fuel 和内存都是每个插件固定配额：一个插件能拿到多少，与其他插件做了什么无关，所以不设先到先得的总池。fuel 是唯一的硬限制，wasmi 没有 epoch，"硬超时"指的就是 fuel。

use crate::abi::Hook;

/// 每个插件的初始化：`msime_abi` 加 `msime_init`。
pub const INIT_FUEL: u64 = 5_000_000;
/// 每次 focus 之后所有插件初始化的合计。超出的插件推迟到下一轮再实例化，不算失败；由宿主一侧记账。
pub const FOCUS_INIT_FUEL: u64 = 5_000_000;
/// 每个插件每一轮（宿主的一次 `msime_client_key` 到下一次之间，跨同一轮内的多次 FFI）。
pub const TURN_FUEL: u64 = 125_000;
/// 单次 `candidates` 的上限。
pub const CANDIDATES_FUEL: u64 = 125_000;
/// 单次 `before_commit` 的上限。
pub const BEFORE_COMMIT_FUEL: u64 = 100_000;
/// 其余观察类钩子单次的上限。
pub const OBSERVE_FUEL: u64 = 25_000;

/// import 每次调用先扣的 fuel。
pub const IMPORT_CALL_FUEL: u64 = 100;
/// import 每复制这么多字节再扣 1。
pub const IMPORT_BYTES_PER_FUEL: u64 = 4;

/// 单个钩子一次最多能用多少 fuel。
pub fn hook_fuel(hook: Hook) -> u64 {
    match hook {
        Hook::Candidates => CANDIDATES_FUEL,
        Hook::BeforeCommit => BEFORE_COMMIT_FUEL,
        _ => OBSERVE_FUEL,
    }
}

/// wasm 的页大小。
pub const PAGE_BYTES: usize = 65_536;
/// 清单没写 `memory_pages` 时的线性内存页数。
pub const DEFAULT_MEMORY_PAGES: u32 = 16;
/// `memory_pages` 的上限：64 页，4 MiB。
pub const MAX_MEMORY_PAGES: u32 = 64;
/// 每个实例的表元素总数上限。
pub const MAX_TABLE_ELEMENTS: usize = 10_000;
/// 每个实例最多几张表（reference-types 允许多张）。
pub const MAX_TABLES: usize = 4;

/// 解释器值栈，整个 Engine 共用（字节）。
pub const MAX_STACK_HEIGHT: usize = 65_536;
/// 最大递归深度。
pub const MAX_RECURSION_DEPTH: usize = 256;
/// 留着复用的栈数。
pub const MAX_CACHED_STACKS: usize = 1;

/// 模块文件的上限。
pub const MAX_MODULE_BYTES: usize = 1024 * 1024;
/// 同时启用的脚本数。
pub const MAX_ENABLED_SCRIPTS: usize = 8;
/// 候选类事件里交给插件的行数上限（窗口内前 20 行）。
pub const MAX_CANDIDATE_ROWS: usize = 20;

/// 进程里全部脚本合计的内存总账：线性内存预留、翻译后的字节码和解释器值栈。授权和启用时按它做准入检查，超出就拒绝启用，而不是让脚本在运行时争抢。
pub const PROCESS_MEMORY_BYTES: usize = 64 * 1024 * 1024;
/// 翻译后字节码相对模块大小的膨胀倍数，按第 3 阶段实测的约 3.3 倍向上取整。准入检查拿不到编译结果，按它估算字节码。
pub const TRANSLATED_BYTES_PER_MODULE_BYTE: usize = 4;

/// 一个脚本在总账里占的字节：一次预留的线性内存，加按模块大小估算的字节码。
pub fn script_memory_bytes(module_bytes: usize, memory_pages: u32) -> usize {
    (memory_pages as usize)
        .saturating_mul(PAGE_BYTES)
        .saturating_add(module_bytes.saturating_mul(TRANSLATED_BYTES_PER_MODULE_BYTE))
}

/// 一组脚本（每项是模块字节数和 `memory_pages`）合计放不放得进总账。值栈整个 Engine 共用一份，只算一次。
pub fn fits_memory_budget(scripts: impl IntoIterator<Item = (usize, u32)>) -> bool {
    scripts
        .into_iter()
        .map(|(module_bytes, memory_pages)| script_memory_bytes(module_bytes, memory_pages))
        .fold(MAX_STACK_HEIGHT, usize::saturating_add)
        <= PROCESS_MEMORY_BYTES
}
