#!/usr/bin/env python3
"""检查 Android 文本控件工厂是否复用基础标签构造。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"
# 文字按钮和标签工厂已陆续挪到 ViewPolicy；Ui 里只剩胶囊按钮这一层，两边一起检查。
VIEW_POLICY = ROOT / "platforms/android/java/app/msime/android/ViewPolicy.java"


def main() -> int:
    errors = []
    source = UI.read_text(encoding="utf-8")
    view_policy = VIEW_POLICY.read_text(encoding="utf-8")
    required = (
        (UI, source, "TextView button = ViewPolicy.centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);"),
        (VIEW_POLICY, view_policy, "TextView button = centeredLabel(context, label, sizeSp, weight, ink);"),
    )
    for path, text, snippet in required:
        if snippet not in text:
            errors.append(f"{path}: 文本控件工厂未复用 styledLabel：{snippet}")

    duplicated = (
        "TextView heading = new TextView(context);\n        heading.setText(text);",
        "TextView button = new TextView(context);\n        button.setText(label);",
    )
    for snippet in duplicated:
        if snippet in source:
            errors.append(f"{UI}: 仍在重复基础标签构造：{snippet.splitlines()[0]}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android text control factories use the shared label constructors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
