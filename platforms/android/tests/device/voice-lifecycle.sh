#!/usr/bin/env bash
# 安装当前工作树的宿主包，仅在专用模拟器上替换识别服务，结束时恢复。
set -euo pipefail
repo_root=$(cd "$(dirname "$0")/../../../.." && pwd)
cd "$repo_root"
android_sdk=${ANDROID_SDK_ROOT:-${ANDROID_HOME:-}}
adb="$android_sdk/platform-tools/adb"
serial=${1:-emulator-5580}
[[ "$serial" == emulator-* ]] || { echo "Only the dedicated emulator is supported" >&2; exit 1; }
avd_name=$("$adb" -s "$serial" emu avd name | tr -d '\r' | head -1)
[[ "$avd_name" == msime-client-test || "$avd_name" == msime-client-test-api* ]] || exit 1
previous=$("$adb" -s "$serial" shell settings get secure voice_recognition_service | tr -d '\r')
restore() {
  # 测试服务不留在可用服务列表里，避免系统在默认项为空时自动选中它。
  "$adb" -s "$serial" shell am instrument -w -e enabled false \
    app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup
  if [[ "$previous" == null ]]; then
    "$adb" -s "$serial" shell settings delete secure voice_recognition_service >/dev/null
  else
    "$adb" -s "$serial" shell settings put secure voice_recognition_service "'$previous'"
  fi
}
trap restore EXIT
"$adb" -s "$serial" install --no-incremental -r target/android/msime-android.apk
bash platforms/android/tests/device/build-editor.sh
"$adb" -s "$serial" install --no-incremental -r target/android/editor-test.apk
setup=$("$adb" -s "$serial" shell am instrument -w -e enabled true \
  app.msime.android.test/app.msime.android.test.SyntheticSpeechSetup)
[[ "$setup" == *MSIME_SPEECH_SETUP_PASSED* ]]
"$adb" -s "$serial" shell pm grant app.msime.android android.permission.RECORD_AUDIO
"$adb" -s "$serial" shell pm grant app.msime.android.test android.permission.RECORD_AUDIO
service=app.msime.android.test.SyntheticSpeechService
api=$("$adb" -s "$serial" shell getprop ro.build.version.sdk | tr -d '\r')
if (( api < 31 )); then service=app.msime.android.test.LegacySyntheticSpeechService; fi
"$adb" -s "$serial" shell settings put secure voice_recognition_service "app.msime.android.test/$service"
result=$("$adb" -s "$serial" shell am instrument -w \
  app.msime.android.test/app.msime.android.test.VoiceRecognitionLifecycleDeviceSmoke)
printf '%s\n' "$result"
[[ "$result" == *MSIME_VOICE_LIFECYCLE_PASSED* ]]
