# Agent Note: 共享界面定时器复用公共间隔 Hook

Status: implemented

## Problem

验证码重发倒计时、鸿蒙输入法切换引导和 macOS 输入法列表重读分别手写 `setInterval`、回调和卸载清理。它们都需要在停用时取消定时器，但没有共享间隔生命周期；回调变化时也容易误把旧闭包留在定时器里。

## Decision

在 `packages/ui/src/core/use-interval.ts` 提供 `useInterval(callback, delayMs)`，接口与现有 `useTimeout` 对齐：`delayMs` 为 `null` 时不注册，间隔变化时清理旧定时器，回调通过 `useLatestRef` 读取最新值。三个调用方继续各自决定何时启用、何时停止以及回调内部的代次和状态更新。

## Alternatives considered

- 每个页面继续保留局部 `setInterval`：改动最小，但会继续复制注册和清理规则。
- 把倒计时、输入法引导和列表重读的状态也收进一个业务 hook：可以减少页面代码，但会把互不相关的启用条件、宿主刷新和验证码代次绑定在一起，破坏各页面的状态边界。

## Verification

新增 hook 测试覆盖正常 tick、卸载清理、最新回调和 `null` 停用；验证码、输入法切换引导及 macOS 输入法列表相关回归测试通过，桌面 TypeScript 检查通过。完整前端门禁和 CI 继续负责其余共享 UI 文件。

## Consequences

共享 UI 定时器统一使用同一套生命周期，回调更新不会重启不必要的 interval。业务 hook 仍负责时间戳计算、请求代次和宿主状态；公共 hook 不替代平台原生定时器，也不改变轮询频率。验证码倒计时的行为边界由[验证码冷却笔记](../bug-fix/2026-10-14-account-verification-cooldown.md)继续持有。

## Existing notes audit

按 `setInterval`、`useInterval`、`useTimeout`、倒计时和轮询检索活跃笔记。验证码冷却笔记记录共享时钟的业务决定，本篇只记录定时器注册清理的 UI 抽取；原生候选、IBus 和 macOS 输入法的定时器笔记属于平台生命周期，未合并。
