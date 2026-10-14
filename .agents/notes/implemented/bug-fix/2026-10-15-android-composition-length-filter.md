# Agent Note: Android 长度限制不把拼音组字算进字符上限

Status: implemented

## Problem

#7841：Android 输入框设置 30 个字符上限时，已有 26 个汉字后输入 `nihao`，输入框把拼音预编辑区也交给 `LengthFilter` 计数。编辑器会保留或截短 `ni`，后续按键看起来像不能输入；最终候选「你好」本来只占两个字符，却无法像其他输入法一样填满剩余空间。

根因在 Android 宿主把预编辑写成真实的 composing region。`EditorInfo` 没有公开编辑器的最大长度，`setComposingText` 也可能在超出限制时返回成功但只保留前缀，因此只看返回值无法判断是否被过滤。

## Decision

每次成功调用 `setComposingText` 后，宿主同步读取光标前最多一段与组字同长的文字，检查它是否以完整组字文本结尾。若编辑器只留下组字前缀，先用空组字清掉被截短的内容，再把这次 Engine 组字标记为「编辑器隐藏」：后续拼音仍由键盘自己的预编辑和候选栏显示，直到候选提交或组字结束。提交候选时重新允许 `commitText`，所以最终汉字按编辑器的字符上限计算。

检查和前缀长度推断集中在无 Android 依赖的 `CompositionTextPolicy`，未知编辑器上下文时保守保留原有内联行为。`SelectionEchoTracker` 记录清掉截短组字后的实际光标位置，避免异步 `onUpdateSelection` 把这次正常回声当成用户移光标。

设备夹具加入一个 30 字符的 `LengthFilter` 输入框：26 个合成汉字后输入 `nihao` 时，组字阶段输入框保持 26 字，空格后得到 30 字的「26 个汉字 + 你好」。

## Alternatives considered

- **始终不把拼音写进 Android 输入框**：能避开所有编辑器长度过滤器，但会失去现有的内联组字、光标点击和读屏语义，也改变已记录的 Android 宿主契约。
- **只在 `setComposingText` 返回 `false` 时回退**：不够，因为 Android `LengthFilter` 常在返回成功的同时静默截短组字。
- **尝试从 `EditorInfo` 读取最大长度**：`EditorInfo` 没有这个字段，应用的 `LengthFilter` 也不会把限制传给 IME。

## Consequences

受长度限制的编辑器在剩余空间不足以容纳完整拼音时，键盘仍显示完整组字，输入框暂时不显示拼音；选词后只写入最终文字。正常编辑器继续使用真实 composing region。若编辑器无法同步读回文字，保守采用原来的内联路径。

## Verification

- `platforms/android/check-host.sh`：230 个 JVM 冒烟、110 个设备套件源码编译和 Android 宿主契约检查通过。
- `EditorSmoke` 新增 `CompositionTextPolicy` 的完整组字、截短前缀和未知上下文用例。
- 设备冒烟新增 `LengthFilter(30)` 的真实 InputConnection 路径；待有 Android AVD 和已验证词库目录时运行 `smoke.sh <serial> --core`。
