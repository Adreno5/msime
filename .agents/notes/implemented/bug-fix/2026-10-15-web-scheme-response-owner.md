# Agent Note: 网页方案切换回复按请求编号归还

Status: implemented

## Problem

网页 SDK `setScheme` 用唯一的 `waitingReady` 保存等待者。连续切换时后一个覆盖前一个；第一条 `ready` 回复结束后一个切换，前一个永不结束。`dispose` 和运行错误同样只拒绝最后一个等待者。另一个交错是全拼切双拼尚未完成时又切回全拼：按已完成的 `scheme` 判断相等会跳过这个真实的返回请求。

## Decision

方案切换和按键共用已有的 `pending` 请求表及 `seq`。Worker 的方案 `ready` 带上请求编号，SDK 只结束该编号的调用；初始加载仍单独等待无编号 `ready`。保留立即发送的调用顺序，只有没有未完成切换且目标等于已完成方案时才跳过。失败和释放通过已有 `fail` 拒绝所有等待者。

## Alternatives considered

- 用 Promise 尾链串行调用可复用单一等待槽，但会把第二次切换的发送推迟到微任务，插在两次调用后的按键可能跑到该切换之前。沿用即时发送和 Worker 队列顺序，不另建调度器。
- 给每次切换新建独立 ready 表也能避免覆盖；但 SDK 已有按编号追踪、统一失败清理的 `pending`，复用它可以避免两套挂起请求契约。

## Verification

每个切换只消费自己的回复；A→B→A 不跳过返回请求；释放与运行失败拒绝全部切换；空闲同方案仍不发请求。可控 Worker 回归在旧实现 4 项失败、1 项通过，修复后通过。直接驱动生产 Worker 的测试核对初始加载无编号、两次方案切换分别带回各自编号；该层只替换 wasm 引擎，消息处理不替换。新增顺序用例核对方案、按键、方案按调用顺序发送且分别收到自己的结果。Node 单元测试 33 项通过，改动 JS 的 lint 与 `git diff --check` 通过。`bash scripts/verify-local.sh --quick` 通过，包含 Rust workspace、Android/Wasm 目标、Linux 设置外壳、Linux 原生 87 项测试、macOS 原生编译和 Apple 桥接；Windows 无本机交叉工具链、HarmonyOS 缺资源时按门禁说明跳过。真实打包冒烟补充连续切换，待 CI 验证；本机缺少匹配版本的 wasm-bindgen 和 wasm-opt。

## Consequences

SDK 与 Worker 需来自同一次包构建，新增编号仅用于包内消息，公共 API 不变。测试不接线上服务，不发布 npm。

## Related notes

- [网页双拼和辅助码](../../implemented/feature/2026-10-08-web-engine-shuangpin-helpcode.md)：规定可就地切换方案及资源保留，本次仅修复并发回复归属，不改变资源和方案集合。
