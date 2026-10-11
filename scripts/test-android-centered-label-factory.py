#!/usr/bin/env python3
"""检查 Android 居中文本是否复用共享标签工厂。"""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
HOME = ROOT / "platforms/android/java/app/msime/android/home"
# 居中标签工厂已从 Ui 挪到 ViewPolicy（签名由 scripts/test-android-view-policy.py 检查），这里核对实现和各页面的调用。
UI = ROOT / "platforms/android/java/app/msime/android/ViewPolicy.java"


def main() -> int:
    errors = []
    ui = UI.read_text(encoding="utf-8")
    required = (
        "public static TextView centeredLabel(Context context, CharSequence text, float sizeSp,",
        "public static TextView centeredLabel(Context context, CharSequence text, int sizeSp,",
        "TextView view = label(context, text, sizeSp, color);\n        setCentered(view);",
        "TextView view = centeredLabel(context, text, sizeSp, weight, color);\n        setSingleLine(view);",
    )
    for snippet in required:
        if snippet not in ui:
            errors.append(f"{UI}: 居中标签工厂缺少：{snippet}")

    consumers = {
        "CloudClipboardPage.java": 1,
        "InputDialog.java": 1,
        "LexiconPage.java": 1,
        "MsToast.java": 1,
        "OnboardingActivity.java": 2,
        "ProfilePage.java": 1,
        "SegmentedControl.java": 1,
        "SheetHeaderView.java": 1,
        "SheetOptionView.java": 1,
        "SkinsPage.java": 4,
    }
    duplicated = re.compile(
        r"(?:Ui|ViewPolicy)\.(?:styledLabel|label)\([^;]{0,300};\s*ViewPolicy\.setCentered\(", re.DOTALL
    )
    for name, minimum in consumers.items():
        path = HOME / name
        source = path.read_text(encoding="utf-8")
        centered = source.count("ViewPolicy.centeredLabel(") + source.count("ViewPolicy.centeredSingleLineLabel(")
        if centered < minimum:
            errors.append(f"{path}: 至少 {minimum} 处文本未复用 ViewPolicy 居中标签工厂")
        if duplicated.search(source):
            errors.append(f"{path}: 仍在标签构造后重复设置居中")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android centered labels use the shared factory")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
