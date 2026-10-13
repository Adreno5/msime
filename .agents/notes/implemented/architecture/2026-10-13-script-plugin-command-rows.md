# Agent Note: 脚本插件在自己的 `/` 指令里出候选：来源 14、按预编辑串缓存的行、只在派发里问插件

Status: implemented

## Problem

脚本插件 v2 的接口只能对内置结果做加工（改写上屏、重排、注释、过滤候选，见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md)），没有任何一条路能让插件自己给出候选：按键不投递，本地模式不在白名单里时文本一律读不到，插件也就做不出 UUID、符号名转字符这类用户真正缺的东西。

v3 的做法是让插件在 `/` 指令模式里注册自己的触发词：用户打 `/uuid` 时由插件给出几行，用户选中其中一行上屏。这要先在引擎和 runtime 里回答几件事：插件的行用什么来源、和内置指令怎么分、列表重建时行怎么不丢、什么时候问插件（不能把打字停顿和网络时序交给脚本）、这些行上屏之后哪些内置机制不能碰它（学习、统计、AI 上文、释义、全角、失焦上屏）。

这篇只记引擎和 runtime 一侧（分支 A）。ABI（能力 `provide_commands`、钩子 `command`、三个 import）和 host-api 的投递（找触发词的主人、有效触发词、随机数）由后续两个分支就地补进这一篇。A 合入之后生产路径上没有东西会调用新代码：`EngineOptions::plugin_commands` 默认为空，`CommandProvider` 没有人挂。

## Decision

### 来源 14

`CandidateSource::Plugin = 14`。13 已经是日文混输行，10 是历史空号不复用。它不属于任何学习谓词（`is_sentence_learning`、`is_dictionary`），也不在 `is_editable_source` 的白名单里，所以：

- 本地模式里选中走 `session/commit.rs` 的本地模式分支，上屏 `item.word` 并 `reset_composition`；本地模式的选中照样调 `learn_candidate`，但整句学习被 `can_learn_sentence_candidate` 的本地模式检查挡住，调词频要 `is_dictionary`，两条都走不到。`/` 只在没有组字时打开，`reset_composition` 清掉了 `ranking_candidates`，学习读的是 `local_candidates`，不会把下标错映射到另一个词库词上。
- 不能置顶、固定或删除。
- 指令模式属于 `generates_text`，不计打字统计。

### 引擎：插件指令、匹配和缓存

- **类型**：`PluginCommand { trigger, title }`（`types.rs`，`CommandTableEntry` 旁边）；`EngineOptions::plugin_commands` 和 `SessionOptions::plugin_commands` 默认空表，`Session::set_plugin_commands` 实时替换。
- **哪些插件指令生效**（`local::command::usable_plugin_commands`）：触发词 2–16 个小写 ASCII 字母，标题 1–8 个字符、不含控制字符、首尾不是空白；触发词不等于、也不和任何内置触发词、用户指令表（`usable_command_table` 之后）的触发词、排在前面的插件触发词互为前缀。用户的指令表永远赢，插件之间先到先得。`CandidateQueries` 留着宿主交来的原表，`set_command_table` 之后按新表重新过滤。过滤之后任何两个生效的触发词都没有前缀关系，所以 `plugin_command_source` 不需要最长匹配，一段输入至多属于一条插件指令。client-core 算有效触发词时要用同一套规则（分支 C），内置触发词由 `local::command::builtin_triggers`、`builtin_titles` 公开（`msime_engine::host::builtin_command_triggers`、`builtin_command_titles`），给分支 B 的一致性测试用。
- **匹配**（`plugin_command_source`）：`/` 之后的输入以某个生效的触发词开头、剩下的全是小写字母时，返回（触发词，参数），参数可以为空。收键本来就只收小写字母（`'` 只在 `/fy` 的单词之间收），本地预编辑串最长 `GENERATED_MODE_INPUT_LIMIT = 64` 字节，参数长度由它封顶。
- **引擎自己不列行**：`CandidateQueries::local` 的指令分支在输入属于插件指令时返回空，列表由缓存的插件行或兜底行（原样的 `/uuid`）顶着。打 `/u` 这样的前缀时插件指令不出现在列表里：列表里每一行都必须能上屏，插件指令在用户打完触发词之前给不出行。
- **要问插件的那一次**（`Session::plugin_command_query`）：只在 `/` 模式里、不在专用英文、输入属于某条插件指令、这份预编辑串还没有缓存时返回 `PluginCommandQuery { session_id, preedit, trigger, argument }`。九键会话一律 `None`。
- **写回**（`Session::apply_plugin_command_rows(query, rows)`）：`query` 必须仍等于当前的 `plugin_command_query()`，否则拒绝（会话不同、预编辑串变了、已经缓存过）。`rows` 是（文本，插件注释），注释不进引擎。引擎丢掉空白、含控制字符、超过 `TEXT_UTF16_LIMIT`（199）个码元、和前面同文的行，最多留 `RESULT_LIMIT`（18）行；行的 `pinyin` 存触发词，来源 14，权重按顺序递减。有行时去掉兜底行；0 行也缓存成空表。
- **缓存**（`InputSession::plugin_rows: Option<(预编辑串, 行)>`）：`update_local_candidates` 每次都从头重建 `local_candidates`，调用它的地方很多（按键、退格、光标处编辑、换表），只记一个「已经问过」的标记的话，标记还在、行已经没了，查询又不会再发。所以缓存的是行本身：重建时在 `apply_candidate_positions` 之后、加兜底行之前，预编辑串和缓存一致就把行放回前面（同文的后一行去掉），不一致就清掉缓存。只有一槽，所以 `/uuid` → `/uui` → `/uuid` 会重新问插件、换一个 UUID；方向键、列表重建时行一直在，不再问。`reset_composition`、进入本地模式、`set_plugin_commands`（授权撤销后旧行不能插回来）、`set_command_table`（生效的插件指令可能跟着变）都清掉缓存。
- **引擎注释为空**：`candidate_annotations` 的指令分支对来源 14 返回空串，`command_title` 也不查插件指令。插件的标题和注释只画在插件注释格里（见下），引擎注释格只放引擎的内容。
- **释义**：`host::glosses::candidate_gloss_key` 对来源 14 返回 `None`，经引擎查释义的宿主路径（macOS 的 `candidate_gloss_request`、Windows 的 `msime_client_candidate_gloss_request`）都不给它查。

### runtime：什么时候问插件

- **`CommandProvider`**（`transform.rs` 的 desktop 模块，cfg 同候选变换）：`provide(trigger, argument, out) -> CommandOutcome`。`Ran` 表示插件运行完了，`out` 就是它的行（可以是 0 行）；`NotRun` 表示插件没运行（模块还在编译、已经熔断、本轮配额用完、没有主人），不写缓存，下一次派发再问。`Runtime::set_command_provider` 挂上或去掉它；移动端和网页引擎没有这个 trait，inert 模块里同名的内部方法什么都不做。
- **调用时机**：`refresh` 的第一步、取快照之前（`run_command_provider`）。只有同时满足才问：正在派发类调用里（`dispatch_live` 置的 `live` 标记）、没有暂停（`suspend_candidate_transform`，宿主在 `COMMIT_OBSERVE_ONLY` 时设）、挂着提供者、引擎的 `plugin_command_query()` 是 `Some`。插件运行了就 `apply_plugin_command_rows`，然后照常取快照。行在同一次派发里写进引擎，代次只在派发开头前进那一次，`CandidateId` 当场就能选。旁路调用（`apply_online_*`、`rerank_settled`、`all_candidates`、`apply_translations`、设置类调用）从不问，引擎缓存的行照样显示，所以停顿和网络时序都不交给脚本；高亮移动和翻页在 refresh 之前就返回，也不问。
- **`Runtime::set_plugin_commands`**：交给引擎，照 `set_command_table` 推进代次并刷新。runtime 不另记标题：宿主交来的表可能有重复的触发词、不合规的标题，哪一条生效只有引擎知道（先到先得、标题不合规的丢掉，用户指令表变了还要重新过滤），runtime 另记一张表就会和引擎分叉，后来者的标题画到先来者的行旁边。
- **插件注释**：runtime 记下最近一次写进引擎的行的（文本 → 注释）；标题按行的 `pinyin`（触发词）向引擎要（`InputEngine::plugin_command_title`，引擎侧是 `Session::plugin_command_title`，查的是过滤之后的生效表）。`script_annotation(seat)` 对来源 14 的可见行返回「标题 注释」，没有注释时只有标题，含控制字符的注释不显示，整段截到 `MAX_ANNOTATION_UTF16 = 32` 个 UTF-16 码元（和 script-runtime 的 `limits::MAX_ANNOTATION_UTF16` 同值，在字符边界上截）。来源 14 的行不经过候选变换的注释表。
- **不进 AI 上文**：`transition` 在 `facts.selection` 的来源是 14、结果是 Committed 时跳过 `remember_commit`。runtime 只能按选中行的来源区分，这是来源 14 不能复用 `Generated` 的原因。
- **全角**：`OutputContext::plugin_row`（`#[serde(skip)]`，不上线缆）按同一条件算；host-api 的 `complete_transition` 在它为真时不转全角，写法同网址模式。同一个 `/` 模式里内置指令（`/rq`）的上屏照旧转全角。
- **失焦**：新动作 `Action::FinishOnFocusLoss`，host-api 的命令 20（宿主替失焦结束组字，macOS 的 `commitComposition:`）派发它，生命周期类别仍是 `FocusLoss`（`InputClass::of` 对它返回 `Commit`，命令 20 再覆盖成 `FocusLoss`）。它和 `Finish` 只差一处：高亮行是来源 14 时走 `Command::CommitRaw`，上屏用户打的 `/uuid`，因为这一次用户没在看候选窗。`Finish`（命令 9）和标点结束仍提交高亮行，和 `/rq` 一致：那是用户自己按下的键。

### host-api 和 client-core 的排除项

- `client-core::translation::source_gets_no_gloss` 加上 14：在线释义不问它。
- `msime_client_translation_query` 在当前页有来源 14 的行时整页返回 `null`：不问在线释义、不交给用户的翻译服务、也不查离线释义，免得插件的文字离开本机或经释义写回学到的词库。页面和回答按下标对齐（`Runtime::translation_candidates`），所以不逐行剔除；这时页上只有插件行，至多还有一行原样的兜底行，整页不请求没有损失。`/fy` 的翻译请求走另一条分支，不受影响。

### Windows 选词策略

`CandidateCompletionPolicy.h` 的 `candidate_finishes_composition` 加上 14。不加也照常出声、画特效：本地模式里选中时引擎 `reset_composition`，这次是 Committed 不是 Partial，`lifecycle::held_piece` 为假。加它是让策略表和引擎「选中即整段上屏」的语义一致，和来源 8 同一个理由。

### 汇总

| 项 | 插件指令行（来源 14） |
|---|---|
| 调词频、记整句、连选成词、个人上下文 | 不做（不属于任何学习谓词） |
| 置顶、固定、删除 | 引擎拒绝（`is_editable_source`），Windows 右键菜单白名单本来就不含 14 |
| 打字统计 | 不计（`/` 属于 `generates_text`） |
| AI 和重排上文 | 不进（`transition` 跳过 `remember_commit`） |
| 释义与翻译 | 整页不请求；引擎的释义键为 `None` |
| 全角 | 不转（`OutputContext::plugin_row`） |
| 失焦（命令 20） | 上屏原样的 `/xxx` |
| 引擎注释 | 空；标题和插件注释画在插件注释格 |
| 简繁转换 | 照做，显示和上屏一致 |

## Alternatives considered

- **插件行插进普通组字的主候选窗**（设计里的方案 B：错音提示、行内拆字）。最强的理由是用户价值更高，不用先进 `/`。不用：全拼、双拼、五笔下 `READING` 是空串，插件拿不到读音；插行要同时改 `mixed_candidates` 和 `ranking_candidates`，否则 `ranking_index` 回退成显示下标、`learn_candidate` 会调到另一个词库词的词频；每个按键都要多跑一次脚本、多取一次快照，时延没量过；五笔四码唯一的判断看的是整个列表的长度。`/` 模式复用 `/fy` 的现成路径，只有本地模式受影响。
- **runtime 在 `set_plugin_commands` 里自己记一张（触发词 → 标题）表**（最初的实现）。最强的理由是 `InputEngine` 不用多一个方法，画注释时也不用跨进引擎。不用：那张表从宿主交来的原表直接收集，重复的触发词后写覆盖前写，标题被引擎丢掉的那条也收进来，而引擎是先到先得，`[{uuid,UUID},{uuid,EVIL}]` 时引擎把 `/uuid` 交给第一条，行旁却画「EVIL」；照着引擎的规则再过滤一遍又得跟着用户指令表变，等于第二份过滤。
- **复用来源 `Generated`（8）**。最强的理由是不用改线缆上的来源编号，宿主一行不改。不用：runtime 只能按选中行的来源区分上屏，复用 8 就分不出哪一次上屏来自插件，UUID 会被喂进 AI 上文，也分不出该不该转全角、该不该请求释义。
- **只记「这份预编辑串已经问过」的标记，不缓存行**。最强的理由是状态最少。不用：`update_local_candidates` 每次都从头重建列表，标记还在、行已经没了，又不会再问，插件行就此消失。
- **缓存多槽，或者只在发出新查询时才替换缓存**。最强的理由是 `/uuid` 退一个字母再打回来时不用再跑插件。不用：那样同一份预编辑串永远是同一个 UUID，和「删掉重打换一个」矛盾；一槽、预编辑串一变就清掉，语义最简单。
- **失焦时也提交高亮的插件行**（和 `Finish` 完全一样）。最强的理由是 macOS 的命令 20 少一个分支，行为和 `/rq` 一致。不用：命令 20 是宿主替用户结束组字，用户这时没在看候选窗；插件行要靠用户自己选中、或者自己按下结束键才进文档。
- **只剔除在线释义、保留离线释义**。最强的理由是离线释义不出本机。不用：页面和回答按下标对齐，保留离线释义要给每一行加「可以离开本机」的标志并改三个宿主的释义路径；插件行没有释义价值，整页不请求更简单。
- **最长匹配，允许插件触发词之间互为前缀**。最强的理由是插件作者有更多触发词可选。不用：`czh` 会截走 `cz` 的输入，后装的插件能抢走先装插件的用户输入；禁止互为前缀之后至多一个匹配，谁拥有输入一目了然。
- **把 `/fy` 的 `'` 分隔推广到插件触发词**。最强的理由是插件可以收多个单词。不用：唯一需要它的是拆字，而拆字已经做成内置的 C 模式（见 [内置拆字反查](../feature/2026-10-13-builtin-chaizi-mode.md)）；插件参数只收小写字母。
- **`CapabilitySet` 先从 u8 扩成 u16**。最强的理由是第 8 项能力正好占满 bit 7，再加一项时 `1 << 8` 会出错。不用：现在只需要 8 项，授权文件存的是能力名，以后扩不影响格式；分支 B 加一条 `Capability::ALL.len() <= 8` 的编译期断言，第 9 项加入时编译失败、在那时再扩。
- **宿主的注释过滤跟着追加新的禁用字符**。见 [脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 里禁用字符那一段：宿主那一道只防断行和方向翻转，runtime 是插件文本的唯一权威。

## Consequences

- **收益**：插件可以在自己的 `/` 指令里给出能选中的行，选中的就是显示的那一行；插件行不学习、不计统计、不进 AI 上文、不请求释义和翻译、不转全角，失焦时不替用户上屏；旁路调用不运行脚本；没有插件指令时引擎和 runtime 的行为不变（有一条测试逐个比较视图）。
- **代价**：`/` 模式多一份缓存和一次匹配；每份预编辑串第一次出现时同步跑一次插件，耗时由 fuel 上限兜住，真机时延要在投递接上之后量。
- **只在 macOS 上验证过失焦路径的来源**：命令 20 只有 macOS 发（`commitComposition:`）。Windows Server 和 Linux 失焦时调 `msime_client_focus(false)`，`Runtime::focus` 在本地模式里丢弃组字、不提交任何行，所以插件行不会被替用户上屏；文档里最后留下什么由各自的框架决定。IBus 对不属于 `commits_on_blur` 的组字用 `IBUS_ENGINE_PREEDIT_CLEAR` 画预编辑（`platforms/linux/src/core/ClientEngine.cpp` 的 `render`），失焦时清掉，什么都不写；Fcitx5 的客户端预编辑是用户打的 `/xxx`，失焦时写不写进文档看 Fcitx5 和客户端，Windows 看 TSF 结束组字时组字范围里的文字，这两处都没有在设备上核实。Fcitx5 在快捷键等路径上发 `MSIME_FINISH_COMPOSITION`，提交的是高亮行，那属于用户主动按键，和标点同一类。
- **触发词规则有两份**：引擎一份，client-core 一份（client-core 不能依赖引擎，见 `docs/ARCHITECTURE.md`），由分支 B、C 的一致性测试守着。
- **重访信号**：真要做「普通组字里插行」（错音、行内拆字）时，上面第一条备选的几个前提都要先解决；需要第 9 项能力时扩 `CapabilitySet`。

## Verification

- `crates/script-runtime/src/tests.rs`：新增的不可见字符逐个经 `rewrite_commit` 和 `candidates_annotate` 得到 `INVALID`，跟在 emoji 后的 VS16 放行。
- `crates/engine/src/local/command.rs` 的 tests：内置触发词和标题的全集；插件触发词和内置、用户表、前面的插件相等或互为前缀时丢掉，长度、字符、标题不合规时丢掉；参数匹配。
- `crates/engine/src/session/tests.rs`：插件行顶替兜底行，0 行时兜底行还在且不再问；方向键和列表重建不丢、不再问，退一个字母再打回来重新问；换插件指令、换指令表清掉缓存，用户表有 `uu` 时 `uuid` 不生效；过期查询被拒；其他指令照旧带标题、从不问插件；开着个人上下文和词频调整时选中插件行只上屏这一行、不带诊断，用户日志行数和词库文件字节不变，不能置顶固定删除；插件行误进 `is_dictionary` 时这条会红（调词频在 `/` 模式下没有键，带回诊断），误进 `is_sentence_learning` 时行为上看不出来（`can_learn_sentence_candidate` 在本地模式一律拒绝），只由谓词单测守着。`types.rs`：`from_u8(14)`、往返覆盖 0–255、不属于任何谓词。`host/tests.rs`：`candidate_gloss_key(_, 14)` 为 `None`。
- `crates/input-runtime/src/tests/plugin_commands.rs`（桌面 cfg，假的提供者）：有插件指令、没有提供者时 `/` 模式的视图和没有插件指令时逐个相同；打完触发词的那次派发里写行，代次只前进一次，ID 可选，引擎注释为空，插件注释是「标题 注释」；表里触发词重复、或前一条标题含控制字符时，行旁的标题是引擎留下的那一条；方向键、翻页、`all_candidates`、`apply_translations`、设置类调用不问；`NotRun` 下次派发再问，0 行不再问；暂停时不问；插件行上屏不进 AI 上文而 `/rq` 照进；注释截到 32 个码元且不切开代理对；`FinishOnFocusLoss` 在高亮插件行时上屏 `/uuid`，`Finish` 和标点上屏高亮行，高亮的不是插件行时和 `Finish` 相同。`tests.rs` 的选词路由表加上 `FinishOnFocusLoss`。
- `crates/host-api/src/tests.rs`：页上有来源 14 时 `msime_client_translation_query` 为 `null`，在线翻译、英文释义、离线释义三种开法都一样，`/rq` 作对照照常请求；全角开着时插件行上屏半角、`plugin_row` 不出现在 JSON 里，同一会话的 `/rq` 照旧转全角。`tests/lifecycle_traces.rs`：命令 20 在插件行高亮时上屏 `/uuid`、记 `commit{FocusLoss}`，命令 9 上屏插件行。`lifecycle/tests.rs`：`FinishOnFocusLoss` 的类别。
- `client-core` 的 `translation` 测试：`source_gets_no_gloss(14)`。
- Windows `tests/input/candidate_completion_policy.cpp` 加 14，只经 CI 的 MinGW 交叉构建编译运行。

相关笔记的当场分类：[脚本插件改写上屏与重排、注释、过滤候选](2026-10-12-script-plugins-modify-commit-and-candidates.md) 部分重叠（禁用字符的事实在那篇就地更新，互链）；[日文混输候选](../feature/2026-10-12-mixed-japanese-candidates.md) 是新来源值不属于任何学习谓词、跳过繁体转换的先例，无冲突；[内置拆字反查](../feature/2026-10-13-builtin-chaizi-mode.md) 解释了拆字为什么不是插件，无冲突；[插件表刷新从不让聚焦失败](../bug-fix/2026-10-11-plugin-table-refresh-never-fails-focus.md) 的规则由分支 C 下发有效触发词时遵守，现在无关。
