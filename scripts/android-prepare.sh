#!/usr/bin/env bash
# Generates the Tauri Android project (src-tauri/gen/android) and applies
# Airspace's Android settings. Requires ANDROID_HOME, NDK_HOME and tauri-cli.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"

if [ ! -d gen/android ]; then
  cargo tauri android init --ci
fi
cargo tauri icon icons/app-icon-1024.png

manifest=gen/android/app/src/main/AndroidManifest.xml
# Health data must not leave the device through Android cloud backup.
if grep -q 'android:allowBackup=' "$manifest"; then
  sed -i 's/android:allowBackup="true"/android:allowBackup="false"/' "$manifest"
else
  sed -i 's/<application/<application android:allowBackup="false"/' "$manifest"
fi
grep -n 'allowBackup' "$manifest"
