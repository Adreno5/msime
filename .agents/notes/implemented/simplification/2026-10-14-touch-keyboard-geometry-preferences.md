# Agent Note: 复用触屏键盘几何偏好读取

Status: implemented

## Problem

设置页主体和拖动预览各自读取 `touch_keyboard_height_adjustment`、`touch_key_spacing_tenths` 和 `touch_row_spacing_tenths`，两处都写同样的空值回落。重复实现容易让默认值或零值语义在后续修改中分叉。

## Decision

共享 `touchKeyboardGeometryPreferences` 统一解析三项几何偏好。读取使用 `??`，因此缺失值回落到 `defaultTouchKeyboardGeometry`，显式的零仍然有效；函数不做范围裁剪，拖动起始值仍在 `pointerdown` 时捕获，拖动中的范围约束继续由现有 hook 负责。Windows 原生设置窗口的同步和取值规则仍由[原生屏幕键盘设置笔记](../feature/2026-10-11-windows-settings-screen-keyboard.md)负责，本次只收拢共享 UI 的读取。

## Alternatives considered

- **继续保留两处内联读取** — 改动最小，但默认值契约仍有两个维护点，后续容易只改一处。
- **让读取函数顺便裁剪范围** — 可减少调用处代码，但读取偏好和编辑中的约束是不同职责，会改变现有对异常存储值的处理边界。

## Consequences

- 共享设置页和拖动 hook 使用同一个几何偏好解析器，显式零和既有默认值保持不变。
- 读取函数新增一个小的公共 UI 模块及单元测试；范围校验、拖动捕获时序和原生宿主同步保持原位置。

## Verification

- 相关 UI 测试和新的读取函数测试共 4 项通过。
- 将继续运行 UI 类型检查、格式检查、笔记门禁和 quick 验证。
