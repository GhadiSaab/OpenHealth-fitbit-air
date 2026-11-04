#!/usr/bin/env bash
# Prepares the Tauri Android project in src-tauri/gen/android.
# The project is committed; this script only regenerates it if missing and
# refreshes the launcher icons. Requires ANDROID_HOME, NDK_HOME and tauri-cli.
set -euo pipefail
cd "$(dirname "$0")/../src-tauri"

if [ ! -f gen/android/app/src/main/AndroidManifest.xml ]; then
  cargo tauri android init --ci
  echo "Regenerated gen/android: re-apply the backup exclusions from git (git checkout -- gen/android/app/src/main)." >&2
fi
cargo tauri icon icons/app-icon-1024.png

manifest=gen/android/app/src/main/AndroidManifest.xml
# Health data must not leave the device through Android backup.
grep -q 'android:allowBackup="false"' "$manifest" || sed -i 's/<application/<application android:allowBackup="false"/' "$manifest"
grep -n 'allowBackup' "$manifest"
