#!/usr/bin/env python3
"""编译执行实际的恢复检查点方法；只替换 Win32/COM 环境，不复制生产逻辑。"""

from __future__ import annotations

import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCE = ROOT / "platforms/windows/tsf/Key/KeyEventSink.cpp"

HARNESS = r'''
#include <algorithm>
#include <atomic>
#include <cstdint>
#include <cstdio>
#include <deque>
#include <string>
#include "InputSchemeTraits.h"
namespace Global { std::atomic<int> InputModeScheme{msime::windows::scheme::Quanpin}; }
using std::min;
using WCHAR = wchar_t;
using SHORT = short;
using WPARAM = std::uintptr_t;
#define _In_
constexpr WPARAM VK_PACKET = 0xe7, VK_LEFT = 0x25;
constexpr int CATEGORY_COMPOSING = 1, FUNCTION_INPUT = 2, FUNCTION_MOVE_LEFT = 3;
constexpr std::size_t MAX_DEFERRED_KEY_DOWN_COUNT = 10;
unsigned LOBYTE(SHORT value) { return static_cast<unsigned short>(value) & 0xff; }
unsigned HIBYTE(SHORT value) { return static_cast<unsigned short>(value) >> 8; }
SHORT VkKeyScanW(WCHAR value) {
    if (value >= L'a' && value <= L'z') return static_cast<SHORT>(value - L'a' + 'A');
    if (value >= L'A' && value <= L'Z') return static_cast<SHORT>(0x100 | value);
    return -1;
}
struct ITfContext {
    int refs = 1;
    void AddRef() { ++refs; }
    void Release() { --refs; }
};
struct CStringRange {
    std::wstring text;
    std::size_t GetLength() const { return text.size(); }
    const WCHAR *Get() const { return text.data(); }
};
struct CompositionState { std::wstring rawInput; std::size_t caret = 0; };
struct Engine {
    CStringRange legacy;
    std::size_t legacyCaret = 0;
    CompositionState current;
    CStringRange &GetKeystrokeBuffer() { return legacy; }
    std::size_t GetCaretPosition() const { return legacyCaret; }
    CompositionState GetKeyCompositionState() const { return current; }
};
struct KeyState { int Category = 0, Function = 0; };
struct DeferredKeyDown {
    enum class Kind { KeyDown };
    Kind kind{};
    ITfContext *context = nullptr;
    WPARAM wParam = 0;
    WCHAR translatedWch = 0;
    unsigned modifiersDown = 0;
    KeyState keyState;
    std::uint64_t focusGeneration = 0;
};
struct CMetasequoiaIME {
    Engine *_pCompositionProcessorEngine = nullptr;
    std::deque<DeferredKeyDown> _deferredAppliedPrefix;
    std::uint64_t _deferredKeyFocusGeneration = 7;
    bool _RefreshDeferredRecoveryPrefix(ITfContext *);
};
'''

TESTS = r'''
int main() {
    int failures = 0;
    const auto check = [&](bool value, const char *message) {
        if (!value) { std::fprintf(stderr, "FAIL: %s\n", message); ++failures; }
    };
    Engine engine;
    ITfContext context;
    CMetasequoiaIME ime;
    ime._pCompositionProcessorEngine = &engine;
    engine.current = {L"nihao", 2};
    check(ime._RefreshDeferredRecoveryPrefix(&context), "checkpoint created");
    const auto &keys = ime._deferredAppliedPrefix;
    std::wstring text;
    std::size_t left = 0;
    for (const auto &key : keys) {
        if (key.keyState.Function == FUNCTION_INPUT) text += key.translatedWch;
        if (key.keyState.Function == FUNCTION_MOVE_LEFT) ++left;
        check(key.context == &context && key.focusGeneration == 7 &&
              key.keyState.Category == CATEGORY_COMPOSING, "keys retain context and focus ownership");
    }
    check(text == L"nihao", "host raw survives an empty legacy buffer");
    check(left == 3, "host caret survives an empty legacy buffer");
    check(context.refs == 9, "each queued key retains the context");

    engine.legacy.text = L"obsolete";
    engine.legacyCaret = 8;
    engine.current = {L"new", 1};
    check(ime._RefreshDeferredRecoveryPrefix(&context), "replacement checkpoint created");
    check(keys.size() == 5 && keys.front().translatedWch == L'n' &&
          keys.back().keyState.Function == FUNCTION_MOVE_LEFT, "current host replaces stale checkpoint and legacy text");
    check(context.refs == 6, "replacement releases previous key references");

    engine.current = {};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty(), "empty host cannot resurrect legacy text");
    check(context.refs == 1, "empty checkpoint releases all references");

    engine.current = {L"abcdef", 0};
    check(!ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty() && context.refs == 1,
          "over-budget host checkpoint is rejected atomically");
    engine.current = {L"abcde", 0};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 10 && context.refs == 11,
          "exact-budget host checkpoint retains text and caret");
    engine.current = {L"A", 99};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 1 &&
          keys.front().wParam == L'A' && keys.front().modifiersDown == 1,
          "caret is clamped and uppercase replay retains Shift");
    engine.current = {L"?", 1};
    check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.size() == 1 &&
          keys.front().wParam == VK_PACKET && keys.front().translatedWch == L'?',
          "unmapped characters retain the packet fallback");
    // 本地 reset 会将这些方案的现有组字上屏，检查点不能再次重放它。
    for (const auto scheme : {msime::windows::scheme::Korean, msime::windows::scheme::Zhuyin,
                              msime::windows::scheme::Vietnamese, msime::windows::scheme::Tibetan}) {
        Global::InputModeScheme.store(scheme);
        engine.current = {L"su3", 3};
        check(ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty() && context.refs == 1,
              "reset-committed composition is not replayed twice");
    }
    check(!ime._RefreshDeferredRecoveryPrefix(nullptr) && keys.empty() && context.refs == 1,
          "missing context clears old references");
    ime._pCompositionProcessorEngine = nullptr;
    check(!ime._RefreshDeferredRecoveryPrefix(&context) && keys.empty(), "missing engine cannot generate replay");
    std::printf("deferred recovery checkpoint: %d failures\n", failures);
    return failures ? 1 : 0;
}
'''


def main() -> int:
    compiler = shutil.which("clang++") or shutil.which("g++") or shutil.which("c++")
    if compiler is None:
        print("skipped: C++ compiler unavailable for Windows recovery checkpoint regression")
        return 0
    source = SOURCE.read_text(encoding="utf-8")
    start = source.index("bool CMetasequoiaIME::_RefreshDeferredRecoveryPrefix(")
    end = source.index("\nvoid CMetasequoiaIME::_ArmDeferredRecoveryForTransport(", start)
    with tempfile.TemporaryDirectory(prefix="msime-deferred-recovery-") as directory:
        root = pathlib.Path(directory)
        unit, binary = root / "recovery.cpp", root / "recovery"
        unit.write_text(HARNESS + source[start:end] + TESTS, encoding="utf-8")
        subprocess.run([compiler, "-std=c++17", "-Wall", "-Wextra", "-Werror", "-DMSIME_EDITION_FULL",
                        "-I" + str(ROOT / "platforms/windows/common"), str(unit), "-o", str(binary)], check=True)
        return subprocess.run([str(binary)], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
