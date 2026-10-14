# Agent Note: 六个平台共用发布构建链的 nightly 产物

Status: implemented

## Problem

Issue #7767 希望用户在正式版之间能下载新提交的可安装构建。当前六个平台的发布 workflow 都是手动触发，且每个平台各自处理资源、原生依赖、架构矩阵和产物校验；如果另写一套 nightly 构建，很容易和正式发布链漂移。

## Decision

为六个正式平台的 release workflow 增加 `workflow_call` 入口，保留原有 `workflow_dispatch`。新增的 nightly workflow 按定时任务或手动触发并行调用 Android、iOS、macOS、Linux、HarmonyOS、Windows 六条现有发布链，统一传入只构建、不创建平台 release 的参数；六个平台都成功后，下载各自的 workflow artifacts，按平台打成一个滚动的 `nightly` GitHub prerelease。iOS nightly 复用现有的签名与外部 TestFlight 流程，上传到外部测试组并提交 Apple 审核；其余平台不使用正式发布密钥或签名服务。

为支持非发布构建，Android 只在 `publish=true` 时强制发布密钥，Windows 只在发布时调用 SignPath，macOS 只在发布时导入 Developer ID 和公证凭据。三个桌面发布 workflow 的官方插件 artifact 改用平台专属名称，避免六个 reusable workflow 在同一个 nightly run 中互相覆盖。

## Alternatives considered

- **复制六套 nightly 构建步骤**：拒绝。依赖安装、资源锁、产物验证和版本矩阵会与正式链分叉，之后修复发布问题时容易漏改 nightly。
- **nightly 直接调用正式发布并创建六个版本标签**：拒绝。正式发布标签按语义版本工作，nightly 重跑会与正式版本冲突，也会触发应用内更新逻辑；滚动的独立 `nightly` prerelease 更适合试用构建。
- **只上传 Actions artifact，不创建 Release**：拒绝。artifact 的保留期和入口受仓库设置影响，用户还要进入具体运行；滚动 Release 给出稳定下载地址，同时保留原始 artifact 供排查。

## Consequences

nightly 与正式发布共享每个平台的构建脚本和验证步骤，六个平台任一失败都不会更新滚动 Release。Release 同时提供六个按平台拆分的压缩包和一个总压缩包，并附统一校验文件。nightly 产物是测试构建：Android 使用开发签名，iOS 同时提供模拟器 artifact 和外部 TestFlight 构建，macOS 可能是 ad-hoc 未公证包，Windows、HarmonyOS、Linux 按各自 workflow 的非发布路径产出；不能把它们当正式升级包。

每次 nightly 会消耗六个平台的 runner，默认每天一次；手动触发用于临时验证。iOS 外部测试构建还受 Apple 审核和 App Store Connect 处理时间影响。滚动 release 替换时会先等六个平台全部成功，随后删除旧的 `nightly` release 并创建新的版本；替换窗口内下载链接可能短暂失效。

## Verification

`actionlint` 检查新增和修改的 workflow 通过，`git diff --check` 通过，`pnpm run verify-notes` 通过；`scripts/run-checks.sh` 的全部契约检查通过或按环境跳过。
