# Agent Note: Fcitx5 网络分发身份独立于缓存探测与显示

Status: implemented

## Problem

按成功显示查询去重仍让云、AI 的空结果和失败每个 tick 重发。AI 缓存命中也被当作成功显示同一查询，反而阻止防抖后的真正网络刷新，用户只能看到缓存答案。

## Decision

每个来源的 `online_dispatched_queries_` 在网络任务成功创建后记录查询。空结果、失败和成功均不对同一查询重复分发；输入查询变化或会话、provider、来源配置失效后重新请求。AI cache-only 探测不写网络分发记录，命中先显示，未命中继续等待空闲防抖，二者均允许一次网络刷新。不新增缓存或 ABI。

原生回包夹具先实际分发，再模拟完成结果；新组合的 AI 用例先消费探测再断言网络分发。隔离 daemon/frontend 用真实合成 socket 检查 `ai_cache_only` 与网络次数，两次组合分别请求一次网络，而不是用 future 存在冒充已请求。离线缺失夹具复用 Fcitx5 `ScopeExit` 恢复资源目录与数据库，正常和异常退出均核对恢复结果。两套真实 daemon 测试复用 `daemon_environment`，统一临时目录与 XDG 状态隔离，避免复制启动夹具后漂移。

## Alternatives considered

- 只对成功显示结果去重，不阻止临时错误后的恢复；但请求失败或空答案会持续重发，而且缓存命中误占网络记录。本次移除这个分发条件。
- 空结果与失败做有界退避，能在用户不改输入时自动恢复；但需要额外计时与重试状态。当前采用每查询一次，若未来明确要求后台恢复，再引入有界退避，不恢复每 tick 重试。
- 保留独立显示查询，再加网络查询和探测完成状态，能分别追踪所有阶段；但当前显示查询除去重外没有用途，已有网络身份和在途槽足够。

## Verification

`fcitx5-online-cloud-empty`、`cloud-failure`、`ai-empty`、`ai-failure`、`ai-cache-hit` 五个独立 D-Bus 场景均先失败：前四项捕获重复网络请求，缓存命中项捕获网络刷新缺失。修改分发后五项通过，每个场景两次组合各有一次网络请求，AI 每次另有一次探测，缓存先显示且随后被新答案替换。

原生云/AI 夹具中途异常恢复断言先失败，加作用域恢复后通过；新组合消费探测后能实际分发网络请求。最终 Linux 全量 CTest 90 项通过，包含新登记的五个分发场景与 IBus 空结果用例；共享 Runtime 177 项与 Host API 428 项通过。原有六个云/AI、延迟和部分翻译回包场景复跑通过。合入上游 `23ba3a6d7` 并补足 AI 空座位释义保留后，Linux 93 项、Runtime 180 项、Host API 432 项及六个隔离翻译场景再次通过，保留上游 Caps Lock 与 AI 座位新测试。所有输入、配置、候选和 provider 都是合成值，不用当前账号或词库输入数据。

## Consequences

网络请求不会因空结果和失败循环重发，AI 保留快速缓存展示与后台刷新。代价是相同查询失败后等待输入或配置变化才重试，不保证无交互的网络恢复。原生与隔离入口验收不等于重新安装后的真实编辑器验收，本次不替换本机插件。

## Related notes

[已显示在线查询去重](2026-10-11-fcitx-online-refresh-translation-flicker.md) 部分重叠：保留宿主分发去重归属，本决定取代其成功显示条件与失败重试策略。[在线候选交集保留](2026-10-11-online-candidate-gloss-retention.md) 和[同代次释义合并](2026-10-11-generation-gloss-merge.md) 负责显示，不承担分发身份。检索命中的[在线批次词条去重](../testing/2026-10-08-online-batch-existing-dedup.md) 处理批内重复，不吸收其规则。
