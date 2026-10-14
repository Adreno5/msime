# Agent Note: 窗口恢复焦点订阅复用公共清理

Status: implemented

## Problem

共享设置页和皮肤目录分别手写窗口 `focus` 与文档 `visibilitychange` 的注册、可见性判断和卸载清理。四处实现容易在某个平台只清理一个事件，或把后台切回前台误当成刷新时机。

## Decision

在 `packages/ui/src/core/window-focus.ts` 提供同步的 `subscribeToWindowFocus` 和 `subscribeToWindowResume`。调用方仍在自己的 `useEffect` 中决定首次读取、启用条件、异步代次失效和轮询；公共函数只拥有事件注册与对称清理。`subscribeToWindowResume` 只在文档状态为 `visible` 时转发恢复事件，并在没有 DOM 时返回空清理函数。

## Alternatives considered

- 每个页面继续维护自己的事件 effect：调用方最直观，但四份注册和清理已经出现，新增页面还会复制可见性判断。
- 把首次刷新、代次递增和轮询也塞进一个 React hook：可以进一步减少 effect 代码，但会把不同页面的异步所有权和请求失效时机绑定在一起，无法保留现有生命周期边界。

## Verification

公共订阅测试覆盖焦点事件、可见与隐藏转换、清理后的无回调；设置页窗口离开测试 5 项通过，外部皮肤、macOS 输入法列表和打字统计相关 11 个测试文件共 147 项通过，桌面 TypeScript 检查通过。改动后 `packages/ui/src` 只保留公共工具本身的 `focus` 注册。

## Consequences

四个调用点共享同一套事件清理和前台判断，首次刷新、请求代次、后台轮询和宿主启用条件仍由各页面控制。公共工具只覆盖同步 DOM 事件，不负责异步注册完成后的取消句柄；那部分契约仍由[窗口状态监听笔记](./2026-10-14-window-state-subscription.md)持有。本次只验证共享 UI 生命周期，不替代各平台原生窗口验收。

## Existing notes audit

按窗口焦点、`visibilitychange`、恢复前台、`subscribeToWindow` 和 `trackAsyncSubscription` 检索活跃笔记。相关的[窗口状态监听清理](./2026-10-14-window-state-subscription.md)只记录异步宿主监听的晚到注册释放，本篇记录同步 DOM 事件订阅；两者边界保持独立。
