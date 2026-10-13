# Punctuation contract

`policy.json` is the authoritative list of ASCII punctuation keys handled by Chinese punctuation mode. `generate.py` emits `policy.h`; consumers use the generated header or, in Rust, `crates/engine/src/punctuation.rs`, whose `matches_the_shared_contract` test compares its table with both files. Nobody copies the mapping into a platform-specific table.

The contract holds stateless simple mappings plus the opening/closing pairs for alternating quotes and nested book-title marks. The quote toggles and nesting depth are session state, so each input session owns that state; the contract provides no shared mutable state.

## 样式（第 2 版）

`styles` 是同一个键的另一组字，不是新键：`cornerQuotes` 给 `"` 和 `'` 换成『』和「」（按键对应，`'` 出「」，Shift+' 出『』），`hollowBrackets` 给 `[` `]` 换成〖〗。`generate.py` 拒绝 `styles` 里出现 `alternating` 或 `simple` 之外的键。用哪一组由偏好 `quote_style`、`bracket_style` 决定：`crates/client-core/src/punctuation.rs` 的 `punctuation_marks` 把偏好（含按繁体输出开关解析的 `corner_when_traditional`）算成两种确定的值，host-api 交给引擎，并在 View 的 `punctuation_marks` 里报告给宿主。交替和嵌套规则对两组字一样，换样式不重置交替。

宿主在文档里做的几步——成对补全、跨过后半个、把前引号改回来、空格转换、英文模式下的中文标点、触屏键面——各自有一张本地表，加字或改字时这些表都要跟着改，否则会出现错位的一对（补出了「却不认识」，于是打出「内容」」）。目前的位置：

- `shared/input/EnglishModeOutput.h`（macOS、IBus、Fcitx5 英文模式下的中文标点，读 View 的样式）
- Windows TIP：`platforms/windows/tsf/Global/Globals.cpp` 的 `PunctuationTable`、`CompositionProcessorEngine.cpp` 的 `GetPunctuation` 与 `SetupPunctuationPair`、`Global/PairedPunctuationHostPolicy.h`、`Composition/Composition.cpp` 的 `SmartPunctuationAsciiFor`、`Key/KeyHandler.cpp` 改回前引号的那一段；样式经配置帧 `PunctuationMarksChanged` 从 Server 推来（`platforms/windows/src/entrypoints/server_main.cpp` 按同一规则从偏好 JSON 算）
- macOS：`platforms/macos/src/input/InputController.mm` 的 `MSIMEPunctuationPairs`、空格转换反查与改回前引号，`platforms/macos/src/core/PairedPunctuation.h`
- Linux：`platforms/linux/src/candidates/PairedPunctuation.h`、`platforms/linux/src/core/SmartPunctuationSpace.h`
- iOS：`platforms/ios/KeyboardExtension/Sources/input/PairedPunctuationPolicy.swift`、`KeyboardViewController.swift` 的 `chineseSymbolFaces`、`KeyboardLayoutTables.swift`
- Android：`keyboard/PairedPunctuationPolicy.java`、`keyboard/ChineseSymbolFaces.java`、`keyboard/KeyboardLayout.java`
- HarmonyOS：`keyboard/input/PairedPunctuationPolicy.ts`、`keyboard/input/SmartPunctuationSpacePolicy.ts`、`keyboard/KeyboardView.ets` 的 `CHINESE_SYMBOL_FACES`、`keyboard/input/NumberSymbolLayout.ts`
- `crates/client-core/src/punctuation.rs` 的 `ascii_for_chinese_mark`（host-api 的空格转换）

After editing `policy.json`, run `python3 shared/contracts/punctuation/generate.py` from the repository root. `scripts/test-punctuation-contract.py` runs it with `--check` in every `scripts/run-checks.sh`, so a header that does not match the JSON fails the gate. The C++ contract test (`shared/contracts/tests/punctuation_contract.cpp`) is built by the Windows project.
