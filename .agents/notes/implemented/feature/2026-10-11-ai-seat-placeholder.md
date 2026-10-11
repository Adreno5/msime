# Agent Note: AI 候选先占座位，回答到达时不挪动其他候选

Status: implemented

## Problem

AI 联想是异步的：宿主在组字停下约 0.65 秒后才发请求，回答再过一次网络往返才到。共享引擎原来把整批 AI 候选（默认 3 条，最多 10 条）插在列表第 2 位，input-runtime 的在线座位（`normalize_online_slots`）再把整组 AI 放在首选和英文之后。回答一到，座位后面的所有本地候选整体后移，用户正看着候选按下的数字键选中了另一个词（#6888）。因为位置规则在共享层，六个宿主都是这样。

## Decision

AI 候选有一个固定的座位，回答到达前先放一行占位，回答到达后只替换这一行。

- 新候选来源 `CandidateSource::AiPending = 12`（追加在枚举末尾，旧值不动，10 是已退役的 NeuralDesktop）。占位行的文字等回答时是「…」，回答为空或失败后是「—」，宿主把来源 12 画成次要色。
- 会话开着 AI 座位（`Session::set_ai_seat`）、当前组字的 `online_query()` 可以问 AI、不在光标前缀解码里、也不是只出单字会把词组滤掉时，`InputSession::reserve_ai_seat` 在引擎候选（混排之前）的第 2 位放占位行，规则与 `replace_online_candidate_batch` 放 AI 候选相同。列表里已有 AI 候选或没有本地候选时不放。
- 座位只坐一行。AI 批次的第一条替换占位行，同一批多出来的排在列表最后：引擎这一层在 `replace_online_candidate_batch`、`rehome_ai_seat` 和固定位置的在线行归位（`apply_fixed_positions_with_state`）里这样排，input-runtime 的 `normalize_online_slots` 把来源 3 和 12 当同一组，只让第一行坐座位，其余接在整个列表（固定位置的英文归位之后）最后。
- 英文行的优先位（`priority_slot`）把占位行当 AI 候选，句子重排模型（`rerank_pick`）不拿来源 3 和 12 比较：在线座位总会把它们放回去，让它们参与只会在回答到达时改变本地候选谁被提到首位。
- 回答为空、失败、被拒，或宿主这次不会发问时，宿主调用 `msime_client_resolve_online_candidates(session, query, length, 1)`，占位行换成「—」，留到组字变化为止（判据是 `OnlineRequestGuard::matches`，代次和查询字段都一样才算同一次）。回答的词全都已在列表里时引擎自己这样处理。
- 占位行不是候选：选它或按它的数字键返回 handled 但不上屏，不学习，不计统计；固定、置顶、删除都不受理；不能从它进入整句改字；收尾时高亮在它上面按首选处理；`candidate_answers_key` 对它为 false。

### 谁打开 AI 座位

Host API 在 AI 请求真的发得出去时打开：AI 开着、接口地址有效、偏好或会话里有可用密钥（`ai_seat_ready`）。macOS 和 Windows 出厂开着 AI 却没有密钥，这时一个请求也不发，若照开关打开座位，每次组字都会留一个永远等不到回答的「…」。桌面 Linux 的密钥只在用户自己的在线服务里，Host API 看不到，只看开关；没有在线服务或在私密输入框里不会发问时由宿主调用 resolve。座位在建会话、偏好更新（不等组字结束）、Engine 重建和 `msime_client_set_ai_credential` 时重算。

### 宿主契约

| 宿主 | 画法 | 何时 resolve |
|---|---|---|
| macOS | 序号色 | 描述符建不出来；回答为空、失败或 apply 被拒（resolve 成功后同一组字不再重发） |
| Windows | 序号色，无右键动作 | `AiCandidateWorker` 对发问了的查询把空回答和异常交给 `SessionController` |
| Linux IBus | 序号色，无主题色时灰色 | 网络请求（不是只查缓存的探测）落空；没有在线服务或私密输入框 |
| Linux Fcitx5 | 斜体、DontCommit（文本格式没有颜色） | 同上 |
| Android | 按钮 0.45 透明度 | 向 AI 发问了却没有采用回答 |
| iOS | 次要色，读屏念「AI 候选等待中 / 没有结果」 | 同上；宿主不让发问（没有完全访问权限）时 |
| HarmonyOS | 次要色 | 向 AI 发问了却没有采用回答 |

同一批修了 macOS 的 `MSIMEViewContainsAICandidate`：它按 `source == 1` 判断，而视图里的 source 是 Engine 的 CandidateSource，1 是用户词库，AI 是 3，候选里有用户词时 AI 缓存一直被跳过。

## Alternatives considered

- **按键后一段时间内不重排。** 最强的理由是不需要新来源、不改任何宿主。但请求本来就在最后一次按键 0.65 秒后才发，危险的时刻是用户读候选的时候而不是按键的时候；长到能防误选的冷却期只会把 AI 再推迟一截。
- **只在宿主里加一个「加载中」提示（例如 macOS 面板末尾一个暗色的 🤖…）。** 改动最小，只碰一个宿主，但它只是提醒用户，候选照样挪位，误选照样发生，其他五个宿主也不变。
- **给 `candidate_limit` 条 AI 候选都预留座位。** 回答到达时完全不动，但默认要占 3 个位置（最多 10 个），没回答或回答少于上限时一页里留一串空座位，本地候选被挤到下一页；决定只留一个座位，多出来的排到末尾。
- **把占位行写进提供者的序列缓存（和 AI 行同一条路）。** 位置天然一致，但缓存按组字持久，回到同一组字会看到早已无关的「…」或「—」，每次按键还要改写一次缓存；占位行属于这一次请求的生命周期，放在会话里按代次判断。
- **失败后去掉占位行。** 列表回到原样，但去掉本身又挪一次位；决定留着「—」直到组字变化。
- **只按 AI 开关打开座位。** 实现最简单，但 macOS、Windows 出厂开着 AI 而没有密钥，所有这些用户每次组字都会多一个永远不会有结果的座位。

云候选这一轮不留座位：它到达时仍会让第 2 位以后的本地候选后移一格（AI 座位跟着它后移，其他行不动）。

## Consequences

- 开着 AI 并配好密钥时，每个可以问 AI 的组字都占一个座位（没有英文、云候选时是第 2 个，有英文时第 3 个），即使最后没有回答。
- 同一批多出来的 AI 候选排到列表末尾，通常不在第一页；`candidate_limit` 默认 3 的用户第一页只看到一条 AI 候选。
- 回答失败后同一组字不再重试（macOS 原来会在下次渲染时重发），组字一变就重新问。
- 引擎回答在线批次的顺序变了：`online_batch.rs`、`positions.rs` 和 input-runtime 的座位测试改了期望，golden 基准里没有多条 AI 的批次，不受影响。
- 没处理：desktop 的落定重排（`rerank_settled`，默认关）在回答到达后会被普通刷新覆盖一次，再在下一次停顿后重排，开着它的用户仍可能看到一次次序变化；Android、iOS 的展开候选面板没有把占位行画暗。
- 与 [英文、emoji 跟着它描绘的词](../feature/2026-10-08-expressive-rows-follow-their-word.md) 部分重叠：英文优先位「有 AI 候选时再往后一位」现在也包括占位行。

## Verification

- `cargo test -p msime-engine`（含 golden）：`online_batch` 的占位行替换、空回答、云候选后到；`positions` 的座位归位；`queries` 的英文优先位；`session` 的占位行出现条件、不上屏、resolve 与代次、回答全是已有词。
- `cargo test -p msime-input-runtime`：`several_candidates_from_one_provider_take_their_seat_as_a_group` 的座位与末尾、真 Engine 上回答前后同页候选不动、数字键不上屏、resolve 后组字变化回到「…」，以及重排模型不比较来源 3 和 12。
- `cargo test -p msime-host-api`：没有密钥不留座位，会话密钥一到就留，resolve 只认来源 1，关掉 AI 立刻去掉占位行。
- macOS `ctest`（149 项）里 `shortcut` 新增回答为空时 resolve 且不重发、视图有用户词时仍用 AI 缓存。Windows 只做了相关文件的 MinGW 语法检查，Wine 套件和真机渲染没跑；Linux、Android 原生、iOS、HarmonyOS 的整包构建和设备验收没跑。
