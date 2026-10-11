# Agent Note: 桌面端生命周期核心：host-api 推导输入事件，内置声音特效与将来的脚本插件共用一条事件流

Status: proposed

## Problem

按键音、上屏音、打字特效和背景音乐的时机现在由四个桌面宿主各自决定：macOS 的 `playKeySound:` 和提交音、Windows Server 的 `FocusedSession::sound_key` 与 `set_music_active`、IBus 和 Fcitx5 各自的调用点。四份实现在细节上已经漂移：IBus 和 Fcitx5 的按键音排在引擎处理之后，Windows 的上屏音按"已送达"算，macOS 失焦上屏也出提交音，Windows 进入密码框时音乐不停。宿主之间唯一共享的是 C ABI 的 transition JSON，而它只说结果（上屏了什么、视图是什么），不说过程（是不是选了候选、选的哪一格、组字是怎么结束的）。

计划中的沙箱脚本插件（观察上屏、改写上屏、重排候选）需要的正是这条"过程"：一个宿主中立、顺序有契约、覆盖所有真实选词路径的事件流。如果没有一个核心先把它定下来，脚本只能挂在各宿主自己的回调上，四个宿主就要各写一份事件顺序。

## Proposal

在 `crates/host-api/src/lifecycle.rs` 建一个生命周期核心，分阶段落地。第 1 阶段（本次）只建核心本身，用户看不到任何变化：没有宿主调用新接口，移动端和桌面的 transition JSON 都逐字节不变。

**事实来自 runtime 的旁路，不改 `Transition`。** `crates/input-runtime` 新增 `Runtime::facts()`，返回 `Copy` 的 `DispatchFacts`，在 `dispatch` 和 `focus` 入口重置：选中的候选（`SelectionFact{seat, source, how, outcome}`，只在 `Finish`、两条标点路由、五笔顶字和唯一码自动上屏、空格落在打开的列表上、数字、14 键原样上屏、点选、以词定字、确认高亮行这些引擎调用点写入，并要求候选文本和引擎的上屏对得上）、撤回暂存选择、暂存段是否增长或随上屏送出、是否原样上屏、按字符数计的上屏尾巴（标点、顶字的键）。另有 `has_composition()`（不看 `snapshot_valid`）和两个 epoch 计数器：高亮移动和翻页在 runtime 里提前返回、不经过 refresh，只比较 refresh 前后的快照会漏掉它们。`Transition` 和它的 JSON 是所有宿主共用的契约，不往里加字段。

**推导是纯函数。** `lifecycle::derive` 只吃调用前后各一次的 `Sample`、派发事实、上屏的字符数和当前页的三项数字，按固定顺序发出事件：`input`（派发前）→ `selection_retracted` → `candidate_selected` → `before_commit` → `commit` → `composition_end` 或 `composition_start` → `composition_update` → `candidates_shown` 或 `candidates_hidden`，focus 的 `focus_in`/`focus_out` 和 destroy 的 `session_end` 排在最后。事件不带文本，按值传递，`size_of::<LifecycleEvent>() <= 24` 由编译期断言守住，推导和按键路径不分配内存（`crates/host-api/tests/lifecycle_alloc.rs` 用只数当前线程的计数分配器断言）。

- `dispatch()` 的顺序：采样并发 `input` → `runtime.dispatch` → 原有统计 → 读事实 → `complete_transition` → `before_commit` 发出点 → 其余事件和内置消费者 → 序列化。`before_commit` 放在 `complete_transition` 之后，脚本将来看到的就是宿主拿到的字节，包括全角转换；尾巴按字符数而不是字节数记，全角把 1 字节的 ASCII 换成 3 字节的 U+FF01..U+FF5E 之后仍切在同一处。本阶段 `before_commit` 只是内部发出点，不改写。
- 一次 transition 同时带着上屏和新组字（韩文音节延续、五笔顶字）时只发 `commit{continues}`，不发 end 和 start。
- focus(false) 的上屏一律不可改写、不出声；focus 的回复不带 `feedback`。
- `apply_online_*`、`resolve/clear_online_candidates`、`apply_cloud_response`、`rerank_settled`、`apply_translations` 这些旁路调用只给核心发视图类事件。将来脚本也收不到它们，否则停顿时长和网络时序会经 `candidates_shown` 泄露出去。`set_english_mode`、`set_chinese_punctuation`、`set_punctuation_lock`、`update_preferences`、`set_ai_credential` 也按旁路调用推导：IBus 的 Ctrl+Shift+E 不先上屏就切专用英文，组字被丢掉；开关 AI 会在组字中途换掉 AI 座位。不推导的话组字的 end 就丢了。
- 派发、focus 和上面这些调用报错时照样推导（`HostSession::lifecycle_on_error`）：失焦先取消组字再刷新，刷新失败时组字已经没了；引擎报错前 runtime 已经刷新过；撤回暂存选择后重打可能失败。报错路径没有上屏，也不报 `candidate_selected`，focus 照样发 `focus_in`/`focus_out`。
- 命令 19 `MSIME_CANCEL_AFTER_HOST_COMMIT` 对会话同 `MSIME_CANCEL`，只在生命周期里把结束原因记成 `HostCommit`。

**选择加入走会话级 FFI，不进 HostOptions。** `msime_client_set_lifecycle(session, flags)` 不落盘，返回实际启用的位；`HostSession.lifecycle: Option<Box<LifecycleState>>` 只由它设上。`HostOptions` 带 `deny_unknown_fields`，每个输入法会话都会重读那份文件，往里加键会让仍在读它的旧版输入法再也开不了会话。另有三个按键路径调用：`msime_client_key`（每个物理按下一次，开启新的一轮；`PASSTHROUGH` 的键不开）、`msime_client_set_field_traits`（`SECURE`、`PRIVATE`、`FULLSCREEN`、`ELEVATED` 等，没有 `KNOWN` 位按未知处理，上屏不可改写；但 `SECURE` 和 `FULLSCREEN` 位本身仍让声音、特效和音乐关闭或静音，宁可多静音一次）、`msime_client_set_active`。顺序契约、PASSTHROUGH 例外和"Windows TIP 会话不调用它们"都写在 `msime_client.h` 里。

**内置消费者只在 `FEEDBACK` 位下生效。** 按键音和按键特效来自 `msime_client_key`，上屏音和上屏特效（typing_effect 事件 4）来自核心的上屏事件，经派发回复的 `"feedback":{"typing_effect":N}` 交给宿主画；背景音乐由 `set_active` 仲裁：进程级 `AtomicU64` 记所有者会话，只有所有者能开关，所有者被销毁时自动关闭，实际状态是"所有者 ∧ 激活 ∧ 非 SECURE ∧ 非全屏"，把 macOS 的 `MSIMEMusicOwner` 下沉到了核心。`FEEDBACK` 下本会话的 `key_sound`、`commit_sound`、`typing_effect`（0-4）和 `music_set_active` 变为空操作；事件 5（绕过按键音的删除）不经过 `msime_client_key`，核心看不到，仍由宿主调用。`SECURE` 字段里没有任何声音和特效，音乐关闭。每个消费者各包一层 `catch_unwind`，panic 只丢掉这一个事件、报告一次诊断，绝不落到 `response()` 那一层把整次上屏变成 `"internal runtime failure"`。

**之后的阶段**：第 2 阶段各桌面宿主接入并打开 `FEEDBACK`，带来五项可感知变化（IBus/Fcitx5 和 Windows 的按键音前移到引擎处理之前、Windows 上屏音从"已送达"提前到"已暂存"、Windows 进入密码框时音乐停止、macOS 失焦上屏不再出提交音、宿主合成的上屏不再算作已上屏），并补上 Windows TIP 到 Server 的字段属性事件；第 3 阶段加 wasmi 解释执行的观察类脚本 hook（`PluginKind::Script` 及其桌面 cfg 骨架、非桌面 `PluginKind::ALL.len() == 8` 的不变量测试都在那一阶段）；第 4 阶段接通 `before_commit` 改写和候选重排、注释、过滤；第 5 阶段是授权页；第 6 阶段是作者工具。脚本只在 macOS IMK 进程、Windows Server、IBus 引擎进程和 Fcitx5 addon 里运行，Windows TIP 和提权窗口里永远不运行；移动端零变化。

## 本阶段的取舍

- **`Pinned` 比"整句改字钉住一段"更宽**：打开的候选列表里一次返回 handled 却不上屏的选择也记 `Pinned`，因为注音列表里的选字就是这样（`su3cl3` 打开列表后按 `3` 钉住"郝"而不上屏）；只限 `Select`、`AnyCandidate`、`Digit`、`Highlighted` 和 `SpaceOverList`。韩文汉字列表也是打开的列表，但它的选择上屏，所以仍是 `Committed`。
- **`DispatchFacts` 多出 `held_flushed`、`raw`、`retracted` 三项**：上屏来源的 `HeldFlush` 和 `Raw` 只看动作本身判断不出来，撤回也只有 runtime 知道。
- **命令 19 只取消一次**：方案里第一次取消只退一层的（韩文汉字列表、藏文、越南文），组字没结束时宿主照旧再发一次，与 Windows 现在对同一个键再发 `MSIME_CANCEL` 的做法一致。在 runtime 里另加一个"彻底丢弃"的动作会改 `Action` 这个公开枚举，本阶段不值得。
- **本阶段不记录轮次计数**：轮次只用于第 3 阶段按轮计的 fuel 预算，现在存一个没人读的计数器只会留下死代码；契约已经写进头文件，宿主从第 2 阶段起照它调用。
- **`input` 事件要读派发前的本地模式**，为此 runtime 新增一个零成本的 `local_mode()` 访问器，不在按键路径上克隆整个 `View`。
- **`PluginKind` 的 cfg 骨架和非桌面 `PluginKind::ALL.len() == 8` 的不变量测试挪到第 3 阶段**：设计把它们列在第 1 阶段，但本阶段不加 `PluginKind::Script`，非桌面的 `ALL` 本来就是 8 项，这时写的测试守的是一个还不存在的 cfg 分支；和 `Script` 变体一起加，测试才第一次有东西可守。本阶段不动 `crates/client-core`。
- **零分配断言要量到内置消费者的上屏分支**，为此 `lifecycle` 导出一个 `#[doc(hidden)]` 的 `replay`：拿给定的采样和事实走一遍真实会话的 `LifecycleState::after`。集成测试是独立的 crate，`#[cfg(test)]` 的钩子到不了；在库的单元测试里装计数分配器又会把几千个无关测试都换到它上面。
- **设置类调用里只给可能在组字中途改动组字或候选的那几个接了推导**。`set_candidate_page_size`、`set_nine_key_mode`、`set_key_grid` 组字中会被拒绝或推迟，`set_character_width`、`set_paired_punctuation` 不碰快照，接上也不会多出事件。

## Alternatives considered

- **把事件推导放进 runtime，或让 `Transition` 多带几个字段。** 最强的理由是 runtime 最清楚"发生了什么"，放在那里不需要旁路接口。不采用：`Transition` 和它的 JSON 是六个宿主共用的契约，加字段就是改移动端也在读的格式；而且声音、音乐所有权、字段属性都是 host-api 的状态（`SessionSound` 和播放器都在这里），runtime 只负责组字编排。旁路的 `facts()` 让 runtime 只陈述事实，解释留给 host-api。
- **用 `is_idle()` 判断组字开始和结束。** 现成、所有宿主都在用。不采用：它把 `snapshot_valid` 也算了进去，一次失败的 refresh 会让一段并不存在的组字"开始"又"结束"；`has_composition()` 只看用户眼里的组字。
- **只靠 refresh 前后的快照比较判断视图变化。** 不需要 runtime 多维护两个计数器。不采用：高亮移动和翻页在 `dispatch` 里提前返回，根本不经过 refresh，`candidates_shown` 会漏掉它们。
- **改写点放在 `complete_transition` 之前，让脚本输出再经过全角转换。** 改写的字节少走一遍转换，脚本也不用关心全角。不采用：脚本看到的就不是宿主最终拿到的文本，pangu 一类插入的半角空格还会被转成 U+3000。
- **选择加入写进 HostOptions 或偏好文档。** 宿主少一次调用，设置也能随偏好走。不采用：`HostOptions` 拒绝未知键，旧版输入法读到新键就开不了会话；生命周期是宿主的接入能力，不是用户偏好，也不该随账号同步。
- **内置消费者在 `FEEDBACK` 关闭时也照常运行，旧调用继续工作。** 迁移期两条路都能出声。不采用：同一个会话里核心和宿主都出声就是响两遍，所以 `FEEDBACK` 下旧调用变成空操作，没开 `FEEDBACK` 的会话完全不受影响。

## Acceptance criteria

- 第 1 阶段：`cargo test -p msime-host-api` 和 `-p msime-input-runtime` 全绿，其中包括拼音（设计 §2.2 的完整轨迹，第一键就开始组字并出候选）、两条标点路由、韩文延续、韩文汉字 Esc、Windows 式"选词后 CANCEL"、命令 19、失焦和 destroy 的黄金轨迹，start/end 成对（含组字中途切专用英文、失焦后刷新失败），SECURE 下零输出，全屏和失焦上屏不要声音，lifecycle 为 `None`（以及选择加入但不开 `FEEDBACK`）时 transition JSON 逐字节相同，全角开启、尾巴是标点时切分正确，必定 panic 的消费者不影响上屏，计数分配器断言按键路径、经内置消费者的上屏和推导零分配，`the_c_header_and_the_rust_exports_agree` 通过；不新增依赖。
- 第 2 阶段起按各宿主 PR 的验收逐条核对，本篇随第一个接入的宿主转到 `implemented/`。

## Risks

- 各桌面宿主的 JSON 解析器能否容忍新增的 `feedback` 键还没核实，每个宿主接入的 PR 都要先加契约测试。
- 移动端目标（iOS、Android、OpenHarmony）本阶段没有交叉编译验证；新代码不带平台 cfg，播放器在这些平台上本来就是空实现，但要等 CI 的平台线确认。
- IBus 两个输入上下文的焦点进出顺序未核实，它直接影响第 2 阶段音乐所有权的验收。

相关：[Windows 打字特效对齐 macOS](../../implemented/feature/2026-10-10-windows-typing-effects-caret-overlay.md) 是第 2 阶段 Windows 接入时要改的那套按键音、特效和 Aux 管道路径。

第 3 阶段的执行器见 [桌面脚本插件的沙箱运行时](2026-10-12-script-runtime-sandbox.md)：脚本 ABI 的 `Hook` 编号沿用本篇 `LifecycleEvent` 的判别值。
