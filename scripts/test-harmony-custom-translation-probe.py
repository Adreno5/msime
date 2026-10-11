#!/usr/bin/env python3
"""鸿蒙设置页「测试自定义翻译配置」与共享层保持一致的守卫。

ArkTS 设置页跑不进 node 单测，这里按文本核对两件事：DeepL 失败提示逐字等于 crates/client-core/src/credential/translation.rs 的 `deepl_failure_message`（Tauri 设置页显示的就是那一份），以及连接测试仍然走共享描述器和共享解析，而不是自己拼请求。
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST_PROBE = ROOT / "crates/client-core/src/credential/translation.rs"
RUST_BUILDER = ROOT / "crates/client-core/src/translation.rs"
SETTINGS = ROOT / "platforms/harmony/entry/src/main/ets/pages/Settings.ets"
NAPI_TYPES = ROOT / "platforms/harmony/entry/src/main/cpp/types/libmsimeclient/index.d.ts"


def block(text: str, start: str, end: str) -> str:
    """从 `start` 所在处截到其后第一个 `end`；找不到返回空串，由调用方报错。"""
    begin = text.find(start)
    if begin < 0:
        return ""
    finish = text.find(end, begin + len(start))
    return text[begin:] if finish < 0 else text[begin:finish]


def rust_messages(source: str) -> dict[str, str]:
    body = block(source, "fn deepl_failure_message(", "\n}\n")
    messages = {code: text for code, text in re.findall(r'Some\((\d{3})\) => \{?\s*"([^"]+)"', body)}
    fallback = re.search(r'_ => "([^"]+)"', body)
    if fallback:
        messages["other"] = fallback.group(1)
    return messages


def harmony_messages(source: str) -> dict[str, str]:
    body = block(source, "private deepLFailure(status: number): string {", "\n  }\n")
    messages = {code: text for code, text in re.findall(r"if \(status === (\d{3})\) return '([^']+)';", body)}
    returns = re.findall(r"^    return '([^']+)';$", body, re.MULTILINE)
    if returns:
        messages["other"] = returns[-1]
    return messages


def main() -> int:
    missing = [path for path in (RUST_PROBE, RUST_BUILDER, SETTINGS, NAPI_TYPES) if not path.is_file()]
    if missing:
        print("harmony custom translation probe: skipped, missing " + ", ".join(str(path.relative_to(ROOT)) for path in missing))
        return 0
    rust_probe = RUST_PROBE.read_text(encoding="utf-8")
    rust_builder = RUST_BUILDER.read_text(encoding="utf-8")
    settings = SETTINGS.read_text(encoding="utf-8")
    napi_types = NAPI_TYPES.read_text(encoding="utf-8")
    problems: list[str] = []

    rust = rust_messages(rust_probe)
    harmony = harmony_messages(settings)
    if set(rust) != {"403", "456", "429", "other"}:
        problems.append(f"could not read the four DeepL messages from {RUST_PROBE.relative_to(ROOT)}: {sorted(rust)}")
    elif rust != harmony:
        for key in sorted(set(rust) | set(harmony)):
            if rust.get(key) != harmony.get(key):
                problems.append(f"DeepL {key} message differs: Rust {rust.get(key)!r}, Harmony {harmony.get(key)!r}")

    # 鸿蒙靠描述器的鉴权头认出 DeepL，这个前缀必须就是共享构造写进去的那个。
    if 'format!("DeepL-Auth-Key {api_key}")' not in rust_builder:
        problems.append("the shared builder no longer writes `DeepL-Auth-Key <key>`; update the Harmony DeepL detection with it")

    probe = block(settings, "private async translationCredential(config: ApiCredentialConfig)", "\n  private ")
    required = {
        "builds the request with the shared descriptor": "client.customTranslationHttpRequest(JSON.stringify(probe))" in probe,
        "rejects a missing descriptor": "descriptor === null" in probe,
        "posts to the descriptor URL": "request.request(descriptor.url," in probe,
        "sends the descriptor body": "extraData: JSON.stringify(descriptor.body)" in probe,
        "keeps the descriptor headers and adds Accept": "const headers: Record<string, string> = descriptor.headers;" in probe
        and "headers['Accept'] = 'application/json';" in probe
        and "header: headers," in probe,
        "detects DeepL by the shared Authorization header": "(descriptor.headers['Authorization'] ?? '').startsWith('DeepL-Auth-Key ')" in probe,
        "parses with the shared parser": "client.parseCustomTranslationResponse(response.result)" in probe,
        "treats ok:false as a failure": "parsed.ok === true && typeof parsed.value === 'string'" in probe,
        "reports DeepL failures by status": probe.count("this.deepLFailure(response.responseCode)") == 2,
        "does not build its own Bearer header": "Bearer" not in probe,
        "NAPI declares the descriptor": "customTranslationHttpRequest" in napi_types,
        "NAPI declares the parser": "parseCustomTranslationResponse" in napi_types,
    }
    problems.extend(f"translationCredential: {name}" for name, present in required.items() if not present)

    if problems:
        for problem in problems:
            print(f"harmony custom translation probe: {problem}")
        return 1
    print("harmony custom translation probe: shared descriptor, shared parser and DeepL messages match the Rust probe")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
