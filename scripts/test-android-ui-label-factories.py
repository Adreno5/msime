#!/usr/bin/env python3
"""检查 Android 文本控件工厂是否复用基础标签构造。"""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "platforms/android/java/app/msime/android/home/Ui.java"
# 文字按钮、胶囊按钮和标签工厂正陆续从 Ui 挪到 ViewPolicy；这里只关心它们复用基础标签构造，不关心落在哪个类里，所以两边合起来检查。
VIEW_POLICY = ROOT / "platforms/android/java/app/msime/android/ViewPolicy.java"


def main() -> int:
    errors = []
    sources = {path: path.read_text(encoding="utf-8") for path in (UI, VIEW_POLICY)}
    combined = "\n".join(sources.values())
    required = (
        "centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);",
        "TextView button = centeredLabel(context, label, sizeSp, weight, ink);",
    )
    for snippet in required:
        if snippet not in combined:
            errors.append(f"{UI} / {VIEW_POLICY}: 文本控件工厂未复用 styledLabel：{snippet}")

    duplicated = (
        "TextView heading = new TextView(context);\n        heading.setText(text);",
        "TextView button = new TextView(context);\n        button.setText(label);",
    )
    for path, source in sources.items():
        for snippet in duplicated:
            if snippet in source:
                errors.append(f"{path}: 仍在重复基础标签构造：{snippet.splitlines()[0]}")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android text control factories use the shared label constructors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
