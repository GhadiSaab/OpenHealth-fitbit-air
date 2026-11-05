# Resume: native Air Bluetooth, 2026-11-05

## Current verified state

- Branch `codex/android-bluetooth`; final diagnostic build14 installed on the Pixel.
- Device credentials remain imported and encrypted; no reimport is needed.
- Final native recording accepts only the provisioned private PSK identity.
- Fixed Golden Gate reset exchange: `80 -> 81 -> 81`. Peer resets invalidate
  the old DTLS context. Added bounded control retries and asymmetric reference tests.
- ATT MTU negotiation now controls fragment size; CoAP empty ACK waits for a
  separate response without retransmitting the acknowledged request.
- Header-only transport diagnostics expose counters, bootstrap classification
  and response codes without keys, identities or health payloads.
- November 5: initial 84 saved measurements became **424** after two private-key
  sessions (59 + 26 readings, four normalized measurements per reading).
  Session 1 continued with the screen off. Build14 session 2 reauthenticated and
  resumed readings after Bluetooth off/on and explicit Start. Private key matched,
  BOOTSTRAP false, CoAP 2.05 Content (69). Stop verified twice, count stayed 424.
- This is a short test, not proof of 30-minute/all-day reliability.
- Radio-free protocol suite passed: reference reset contract, CoAP, strict PSK
  (including rejection of the known public bootstrap key), both CCM variants,
  and DTLS-over-Gattlink at MTUs23/247 with packet loss and sequence wraparound.
- Original Google Health package enabled state0, Nearby permissions and 30-second
  screen timeout restored and checked. Airspace recording is stopped. Bluetooth on.
- Last wireless ADB endpoint `192.168.1.172:38981`; may change on reconnect.

## Remaining limitation / next focused work

Google Health can contend for the Air link. Release its Bluetooth access while
using Airspace. Some failed attempts presented BOOTSTRAP instead of the owner's
private identity. A temporary public-key diagnostic completed encryption but
could not read live data; public resolution was removed from the final build.
Briefly allowing Google Health to refresh, then releasing its link, restored
private-identity use once. No guarantee this workaround is always sufficient.

Next: test a longer session and recovery from an ordinary BLE disconnect without
turning off the phone's adapter; capture only sanitized failure metadata if it
fails. Do not revisit disproven cipher/session-ID experiments or broaden key
resolution. Account provisioning, renewal and Google-backed history are separate
unimplemented work; never infer sleep, HRV, SpO2 or temperature from liveactivity.

## Local evidence and helpers

- `.tools/pixel-build13-*.json`, `.tools/pixel-build14-*.json`: sanitized test snapshots.
- `.tools/protocol-final.log`, `.tools/android-build-14.log`.
- `.tools/pixel-control.mjs`: own-app start/stop/status through WebView CDP.
- `.tools/*restore.json`: original temporary phone settings; restore_required=false.
- Credentials and tools remain ignored. Original research under
  `/home/ghadi/perso/fitbit-air-research` was not modified.

## Build environment

- JDK: `/nix/store/56mv72maqaj8j241d98fjcvwswaipfay-openjdk-21.0.12.1+1`.
- SDK: this checkout's `.tools/android-sdk`; installed platform is `android-37.0`
  (not the old README's unavailable `platforms;android-37` package).
- NDK: `.tools/android-sdk/ndk/27.2.12479018`.
- CLI tools installed: tauri-cli 2.12.4, trunk 0.22.1, wasm-bindgen-cli 0.2.130.
- ADB: `/nix/store/hx0ablqbnhgy5d9l1br852rvv13h50jp-androidsdk/libexec/android-sdk/platform-tools/adb`.
  Use this 37.0.1 binary: the older PATH adb failed to connect to wireless ADB.
- Build: set JAVA_HOME, ANDROID_HOME, ANDROID_SDK_ROOT and NDK_HOME; then
  `(cd app && trunk build --release)` and
  `cargo tauri android build --debug --apk --target aarch64`.
- PWA smoke: NODE_PATH points at `.tools/node_modules`; set
  PLAYWRIGHT_CHROMIUM_EXECUTABLE to the system Chromium path.
- JVM tests: use JDK's java/javac, `.tools/protocol-jars/*` classpath and the
  protocol main/test Java source directories. Dependencies are already local.

README feature tables predate this implementation; use this handoff for actual
status. Historical converter, independent provisioning/renewal and offline
historical decryption are not part of the completed connection work.
