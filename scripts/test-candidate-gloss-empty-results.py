#!/usr/bin/env python3
"""锁定无法在 Linux 实例化的宿主空回包接线；不替代各平台原生测试。"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class EmptyGlossResultWiring(unittest.TestCase):
    def test_macos_empty_result_keeps_service_fallback(self):
        source = (ROOT / "platforms/macos/src/input/InputController.mm").read_text()
        body = source.split("- (BOOL)applyCandidateTranslationResults {", 1)[1]
        body = body.split("- (void)stopAccountGloss {", 1)[0]
        guard = body.index("if (!results.count) {")
        apply = body.index("[_session applyTranslations:")
        empty = body[guard:apply]
        self.assertIn('_translationAppliedGeneration = view[@"generation"];', empty)
        self.assertIn("[self synchronizeCandidateServices];", empty)
        self.assertIn("return YES;", empty)
        self.assertIn("applyTranslations:@[]", source)

    def test_android_empty_result_follows_offline_bookkeeping(self):
        source = (ROOT / "platforms/android/java/app/msime/android/core/MSIMEInputService.java").read_text()
        body = source.split("private void applyCandidateGlosses(", 1)[1]
        body = body.split("NativeClient.applyTranslations(", 1)[0]
        guard = body.index("if (new JSONArray(translations).length() == 0) return;")
        self.assertLess(body.index("candidateOfflineGlossGeneration = token.generation();"), guard)
        self.assertIn('session, generation, "[]"', source)

    def test_ios_empty_offline_array_never_reaches_session(self):
        source = (ROOT / "platforms/ios/KeyboardExtension/Sources/keyboard/KeyboardViewController.swift").read_text()
        body = source.split("let applyOnMain: (UInt64, Data) -> Void", 1)[1]
        body = body.split("queue.async {", 1)[0]
        guard = body.index('guard translations != Data("[]".utf8) else { return }')
        self.assertLess(guard, body.index("self.session.applyTranslations("))
        self.assertIn("self.candidateGlossEpoch == targetEpoch", body)


if __name__ == "__main__":
    unittest.main()
