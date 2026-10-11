#!/usr/bin/env python3
"""Android theme ripples use the shared drawable policy directly."""

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
ANDROID = ROOT / "platforms/android/java/app/msime/android"
POLICY = ANDROID / "DrawablePolicy.java"
UI = ANDROID / "home/Ui.java"

def main() -> int:
    errors = []
    policy = POLICY.read_text(encoding="utf-8")
    for snippet in (
        "public static RippleDrawable ripple(int color, Drawable content, Drawable mask)",
        "return new RippleDrawable(ColorStateList.valueOf(color), content, mask);",
    ):
        if snippet not in policy:
            errors.append(f"{POLICY}: 缺少 {snippet}")
    view_policy = (ANDROID / "ViewPolicy.java").read_text(encoding="utf-8")
    if "public static Drawable ripple(Context context)" not in view_policy:
        errors.append(f"{ANDROID / 'ViewPolicy.java'}: 缺少主题 ripple 工厂")
    ui = UI.read_text(encoding="utf-8")
    if "Ui.ripple(" in ui:
        errors.append(f"{UI}: 仍保留 Ui ripple 转发调用")
    # ViewPolicy 要在不带 AndroidX/Material 的 check-host.sh 里编译，不能引用 DrawablePolicy，只能自己构造圆角波纹（见 .agents/notes/implemented/bug-fix/2026-10-11-android-host-compile-regressions.md）。
    framework_only = ANDROID / "ViewPolicy.java"
    for path in ANDROID.rglob("*.java"):
        source = path.read_text(encoding="utf-8")
        if path not in (POLICY, framework_only) and "new RippleDrawable(" in source:
            errors.append(f"{path}: 应复用 DrawablePolicy.ripple")
        for forbidden in (
            "DrawablePolicy.ripple(this)",
            "DrawablePolicy.ripple(activity)",
            "DrawablePolicy.ripple(view.getContext())",
        ):
            if forbidden in source:
                errors.append(f"{path}: 主题波纹应复用 ViewPolicy.ripple")
        if path != UI and "Ui.ripple(" in source:
            errors.append(f"{path}: 不应调用已删除的 Ui ripple 转发")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("Android ripple drawables use the shared factory")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
