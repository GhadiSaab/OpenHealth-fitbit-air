# Resume: native Air Bluetooth, 2026-11-04

**Paused at the owner's explicit request. No further experiments until asked.**

## Where we stopped

- Repository: `/home/ghadi/perso/OpenHealth-fitbit-air`.
- Branch: `codex/android-bluetooth`, based on `codex/direct-bluetooth-plan`.
- The Android APK is installed and launches on the unrooted Pixel 10 Pro,
  Android API 37, package `app.airspace.fitness`.
- The owner key was imported with Android's document picker and encrypted in
  Android Keystore-backed preferences. The temporary Downloads credential file
  was deleted. Do not ask the owner to provision the same key again.
- Direct reads really succeeded: one attempt saved **8 raw readings / 32
  normalized personal-database measurements**, fields 1–10 present. No Google
  history relay or new Google authorization was used.
- The connection is **intermittent**. Other attempts complete GATT/cache setup
  and Gattlink, then stall at DTLS authentication and reconnect. Do not call
  sustained recording reliable or complete.
- Stop was issued and verified: `state=disconnected`, `recording=false`;
  **32 personal measurements remain saved**. The latest session had 0 samples;
  its zero does not erase the earlier successful session's records.
- Google Health (`com.fitbit.FitbitMobile`) was force-stopped to release BLE.
  Pairing and app data were preserved. It has not been reopened automatically.

## Next investigation (start here)

1. Reconnect wireless ADB (last address `192.168.1.172:35309`; port may change).
2. Read this handoff and the implementation ledger before changing anything.
3. Start the installed app/service and inspect sanitized `transport` counters.
   During a stalled attempt: `ip_input=2`, `dtls_input=2`, `dtls_output=6`,
   `last_record_type=22`, thread `TIMED_WAITING`. Air sends handshake records
   and the server responds; this is not simply failure to discover BLE.
4. Diagnose why the DTLS flight sometimes fails to complete. Next useful
   observation is Gattlink outstanding/pending/ACK counters and the TLS flight
   stages, without logging keys or plaintext health packets. Validate against
   Golden Gate's reference framing; do not guess cryptographic changes.
5. Once repeatable, test background/screen-off, reconnect and Stop. These are
   **not yet verified**. No 30-minute stable recording test has passed.
6. Complete remaining CoAP empty-ACK handling: currently the poller preserves
   token/MID and ACKs separate CON replies, but still retransmits a CON request
   after an empty ACK while waiting for the separate response.

## Implemented

- Native Tauri Android plugin: `crates/tauri-plugin-air`.
- Kotlin permission/credential-picker bridge; Java foreground service, serialized
  GATT, notification Stop, retry cancellation epochs, wake lock and durable spool.
- Application-owned bounded Gattlink, IP/UDP, CoAP and protobuf code.
- Bouncy Castle DTLS 1.2 PSK server: bctls 1.86.2, bcprov/bcutil 1.86.
  Golden Gate's archived Android build was not imported. Initial CMake probe
  lacked its external TLS sources; no claim it is impossible to modernize.
- Strict provisioned PSK identity; no fallback for arbitrary identities.
- `live_ingest.rs`: bounded values, metres conversion, timestamp fallback,
  provisional mappings, session/link boundaries and replay IDs.
- Native Rust spool ingestion: personal DB only, commit before acknowledge,
  durable dirty-summary markers, lifecycle mutex shared with deletion.
- Device controls and current-reading panel; browser/demo remain separate.

## Verification already completed

- Core suite: 47 unit + 1 existing integration + 7 live-ingestion tests = 55 pass.
- JVM protocol tests pass: malformed inputs, duplicate/lost fragments, retransmit,
  default ATT <=20-byte frames, late CoAP replies/token retention.
- Synthetic DTLS PSK exchange passes; unknown identity rejected.
- Two gap regressions failed before the fix (+10 invented steps), then passed.
- Android Java and Rust compiled; debug APK build passed (deprecation warnings).
- APK signature verified with apksigner's JAR (the downloaded shell launcher
  references `/bin/bash`, absent here).
- Complete existing PWA browser smoke test passed using system Chromium.
- Independent code review fixes applied: ATT size, CoAP outstanding token,
  Start/delete serialization and queued-start cancellation, counter boundaries,
  per-attempt freshness. Hardware reliability remains unresolved.

## Artifact and local evidence

- Installed latest diagnostic APK:
  `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`
- SHA-256: `f59675f7942d3bacbc9c711d88129a5be9c8069181f2427644e6d5c4b91290f0`
- `.tools/android-build-4.log`, `.tools/core-green.log`, `.tools/pwa-smoke.log`,
  `.tools/pixel-connection-test.json` (first 40 s, before the later success).
- `.tools/pixel-control.mjs` can query/start/stop through the app's WebView CDP.
  It reconnects the ADB WebView forward using the current app PID; update the
  device address there if necessary. Returns sanitized diagnostics and counts.
- Local credential import source remains ignored under `secrets/` mode 700,
  file mode 600. Never commit or print it.
- Original research: `/home/ghadi/perso/fitbit-air-research`; keep untouched.
- `.superpowers/sdd/2026-11-04-android-bluetooth/progress.md` is the local ledger.

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
