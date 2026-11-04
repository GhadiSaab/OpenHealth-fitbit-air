# Android Bluetooth implementation plan

**Goal:** Build a native Air connection and test it on the owner's Pixel.
**Spec:** ../specs/2026-11-04-direct-bluetooth-design.md
**Execution:** Native execution, authorized by owner 2026-11-04.

## Constraints

- Keep the Rust/Leptos/Tauri app and existing pairing.
- Never commit owner credentials or health data; secrets remain native.
- No historical sync completion or history deletion requests.
- Report actual device results separately from simulator/unit results.

## Tasks

### Task 1: Protocol build proof

- [ ] Establish Android build tools and ADB access.
- [ ] Evaluate Golden Gate build compatibility; use a maintained DTLS-PSK
  implementation if its obsolete Android toolchain blocks integration.
- [ ] Add JVM protocol tests first: protobuf truncation, IP/UDP validation,
  Gattlink loss/duplicate/wrap handling, CoAP token matching, strict PSK identity.
- [ ] Implement pure protocol transport and verify DTLS interoperability with
  synthetic credentials before any owner key is provisioned.

### Task 2: Native recorder

- [ ] Add Tauri mobile plugin with permission handling and system credential picker.
- [ ] Add foreground service, serialized GATT, notifications and Stop.
- [ ] Persist readings and state independently of the webview; replay safely.
- [ ] Verify native compilation and unit tests.

### Task 3: App wiring and ingestion

- [ ] Test normalized live sample ingestion and deterministic replay IDs.
- [ ] Implement native personal-database reconciliation and UI snapshots.
- [ ] Wire Device and Today to real service state; preserve demo isolation.
- [ ] Run core, WASM and Android build checks.

### Task 4: Pixel validation

- [ ] Verify signed APK, install without clearing existing app data, provision
  credentials privately and grant BLE permissions.
- [ ] Verify authenticated live readings, background recording, Stop and reconnect.
- [ ] Record duration, gaps, limitations and exact tested artifact in the repository.

## Review focus

Malformed radio packets, unknown PSK identity, UI absence during collection,
duplicate spool replay, Stop/delete racing a reconnect, and stale UI status.
Any untested behavior stays explicitly unverified.
