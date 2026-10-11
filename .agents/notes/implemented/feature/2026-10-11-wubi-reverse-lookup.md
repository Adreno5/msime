# Agent Note: 五笔反查：`z` 引导临时拼音

Status: implemented

## Problem

#6789：五笔用户想用拼音找到一个字，同时看到它的五笔编码。引擎早就会给五笔方案里的候选附上反查编码（`ImeSession::refresh_wubi_codes` 查 `wubi86`/`wubi98` 里最长的码，`host/session.rs` 把它接在注释位上），拼音候选也一样附码，但拼音只能从「五笔拼音混输」进来。完整版默认关掉混输，关掉以后五笔方案里没有任何键能打出拼音：`WubiScheme` 拒绝 `z`，其余字母都被当成五笔码，最多四个。

## Decision

混输关闭时，五笔组字开头的 `z` 打开临时拼音反查，默认可用，没有单独的开关。

- **为什么是 `z`**：五笔 86 和 98 的码都只用 `a`–`y`，`z` 在五笔方案里没有任何用途。引擎里没有五笔万能键或学习键：`WubiScheme` 在混输关闭时直接拒绝 `z`，交回宿主原样输入。极点五笔、QQ 五笔和 Rime 的五笔方案都用 `z` 引导拼音反查，用户的手感和文档也都按这个来。
- **只在组字开头**：`z` 只有在组字为空时引导反查。五笔码中间的 `z`（`wq` 之后）照旧拒绝、交回宿主。混输打开时 `z` 本来就是拼音字母，所以 `ImeSession::bind_wubi_scheme` 让反查和混输互斥：`set_pinyin_lookup_allowed(!mixed_pinyin)`。
- **状态放在组字里，不另起本地模式**：反查组字仍是 `SchemeType::Wubi` 的组字，原文就是键入的 `z...`，`local_mode` 保持 `none`。`WubiScheme::in_pinyin_lookup` 只看「允许反查且原文以 `z` 开头」，退格删掉 `z`、取消、上屏都自然结束反查。`ImeSession::decode` 遇到反查组字时不查五笔码表，把 `z` 后面的字母交给混输共用的 `QuanpinScheme`，只出全拼候选。反查组字最多 32 个字母，与混输的上限相同。
- **编码提示照旧**：反查的候选都是全拼行，组字仍在五笔方案、`local_mode` 是 `none`，所以 `refresh_wubi_codes` 和 `host/session.rs` 现有的规则直接给它们附上当前码表版本（86/98）的五笔编码。单字显示的是最长的全码（你 → `wqiy`，不是简码 `wq`），沿用 `REVERSE_QUERY_SQL_86`/`_98` 的取法。
- **选词按拼音推进**：选走一个只覆盖一部分拼音的候选后，剩下的拼音仍在反查里。`InputSession::selection_completes_composition` 和 `advance_composition_after_selection` 只在 `z` 后面那段拼音里计算消耗了多少字母，再把 `z` 接回剩余原文前面；不接回去，剩下的 `hao` 会被当成五笔码。回车（`CommitRaw`）和没有候选时结束组字（`finish`）上屏的是键入的拼音，`z` 与临时英文的 `Y` 一样只是标记，不上屏。
- **不学习**：反查只是查编码。反查里的选择不调词频、不记整句（`learn_candidate` 直接返回），多段选择也不造词（`commit_selection` 的 `store_user_phrase_from_canonical_pinyin` 跳过）。五笔码表从来不会因为反查写入；拼音词库也不会，否则一个五笔用户偶尔查一次字，就会改掉他在拼音方案里的候选顺序。
- **宿主不用改**：快照里反查就是一个五笔组字，`answered_by_pinyin_fallback` 为真，所以 `input-runtime` 的四码顶字（`wubi_four_code_is_complete`）和四码唯一自动上屏都不会对 `zhao` 这样的四个字母动手。各宿主本来就把五笔里的小写字母交给引擎，由引擎决定收不收，所以 Windows、macOS、Linux、Android、iOS、HarmonyOS 不需要各自加一份判断。

## Golden 基准

三个场景原本记录的是参考实现「五笔里 `z` 被拒绝」的行为，现在按新行为更新：`wubi_mixed_plain_mode` 和 `wubi_mixed_matched_code` 在混输关闭时键入的 `zi` 记录为反查组字 `zi`；`wubi_select_and_gates` 把 `z` 的拒绝挪到五笔码中间（`wqb` 之后）检验，四码、第五码和撇号的检验不变。`tools/engine-golden/gen_scenarios.py` 同步改了这两个场景的步骤与说明。

## Alternatives considered

- **新增一个 `LocalInputMode`（像 `R` 临时日文那样切换方案）**：最强的理由是隔离得干净，预编辑可以显示成独立的模式，选词和学习各走本地模式那一套，不碰五笔组字的代码。没有采用：`LocalInputMode` 的名字是跨进程契约，六个宿主都有各自的模式标签、打字统计和中文输出策略按名字分支（Linux `CandidateLocalModeLabels.h`、Android `LocalInputMode.java`、Harmony `LocalInputMode.ts` 等），新增一个值要在每个宿主补一遍；本地模式的选择总是整段上屏，也就没法选走一部分拼音再接着反查。留在五笔组字里，宿主侧一行都不用改，现有的编码提示规则直接生效。
- **只告诉用户去打开「五笔拼音混输」**：不改代码。没有采用：混输会让每个五笔码后面都跟着拼音候选，打不出的码还会延长到拼音；只想偶尔查一个字的五笔用户不该为此改变整个输入手感。
- **加一个反查开关**：可以让不想要的人关掉。没有采用：`z` 在混输关闭的五笔里原本没有任何作用，占用它不会挡住任何五笔码；开关要进共享偏好、账号同步、MCP 偏好和六个宿主的设置界面，而它保护的只是「误按 `z` 时原样输入一个 `z`」，这时按退格或回车（回车上屏的是 `z` 之后的字母，单独的 `z` 什么也不上屏）即可。真有用户要求关掉，再按 [四码唯一自动上屏开关](2026-10-08-wubi-auto-commit-unique-preference.md) 那条路接进共享偏好。
- **反查里照常学习**：选过的拼音词下次排前面，查同一个字更快。没有采用：学习写进的是拼音词库，五笔用户在反查里的一次选择会改掉拼音方案（和混输）里的候选顺序；反查查的是编码，不是输入习惯。

## Consequences

- 混输关闭的五笔用户按 `z` 加拼音就能看到候选和它们的五笔编码，跟随所选的 86/98 码表；混输打开的用户行为不变。
- 代价：混输关闭时，五笔里单独按 `z` 不再原样输入 `z`，而是开始一个反查组字。反查的预编辑显示键入的原文（`znihao`），没有拼音分词。反查不支持撇号分隔，这一点与混输相同。
- 反查的拼音候选按拼音词库原有的顺序排列，不受个人学习影响，也不留下学习记录。

## Verification

- `cargo test -p msime-engine --lib`：`wubi::scheme` 新增三条（开头 `z` 引导、反查上限 32 与四个字母不是完整码、中间 `z` 仍拒绝、退格离开；不允许时 `z` 被滤掉；宿主编辑整段保留反查原文），`host::tests` 新增六条：反查附码且单字是全码、跟随 98 码表、进入与离开（退格、取消、离开后五笔码照常）、混输里没有 `z` 反查、选走一部分后剩余拼音仍在反查且回车只上屏拼音、反查不学习（并以混输里同样的选择会写日志作对照，证明测得到学习）。不学习那条在去掉 `learn_candidate` 的判断后失败。
- `cargo test -p msime-engine --test golden`：更新后的三个场景与其余场景全部通过。
