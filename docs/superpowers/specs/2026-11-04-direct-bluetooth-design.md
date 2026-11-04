# Direct Fitbit Air recording: integration design

Status: proposed implementation design; no radio or authentication experiment
was run for this change. Product code is unchanged. Owner request: investigate
the connection on a separate branch, building on the already demonstrated
direct Bluetooth client.

## Outcome and scope

On the owner's unrooted Pixel, import previously obtained credentials once,
select the Air, start recording, and retain live readings locally while the
screen is closed. Import already processed historical records separately.
The normal live path requires neither Google sign-in nor a desktop helper.

This is personal-device onboarding, not a solution for provisioning another
owner's device. Credential renewal is unresolved. New sleep stages, SpO2,
HRV, respiratory rate, skin temperature, resting-heart-rate summaries and
Fitbit zone summaries remain history-import features. Do not derive them
silently from the four live fields.

## Repository audit

Inspected base: `cfa4e8575687ebe008c8e006865013f1199ab59f`, default branch
`claude/sharp-allen-fufwmp`. This app uses Rust/Leptos and Tauri 2, not
React/Capacitor; keep its existing architecture.

| Existing component | Integration consequence |
| --- | --- |
| `crates/airspace-core/src/bluetooth.rs` | Reuse `LiveEvent`, `LiveSample`, `ConnectionState` and capability/freshness concepts; extend for protocol failure and recording state as needed. |
| `app/src/screens/profile.rs` | Device action is unavailable; replace it with real setup/start/stop controls only on Android. |
| `app/src/screens/today.rs` | Status is hard-coded; HR queries stored samples and passes `connected=false` to freshness. Add current-reading signals and actual connection state. |
| `app/src/state.rs::App::ingest` | Uses the currently selected database and runs in the webview. Never use this as the recording service's persistence path. |
| `src-tauri/src/db.rs` | Reuse native personal SQLite and deterministic measurement IDs. Add native ingestion/reconciliation independent of UI. |
| `crates/tauri-plugin-vault` | Existing Keystore AES-GCM pattern; extend native-only credential access. Do not expose a generic secret getter to the webview. |
| Android manifest | Currently only INTERNET; BLE permissions, foreground-service declaration and service implementation are absent. |
| `aggregate.rs` | Counter differencing exists, but source priority and gap handling require specific live integration tests. |
| `airspace.import` version 1 | A research readback is not this format; it needs an explicit converter. |

Baseline check: `cargo test -p airspace-core --locked` passed: 47 unit tests
and one integration test. This does not establish an Android APK build or
Bluetooth operation on the phone.

## Approaches and recommendation

1. **Native Android recording service through a Tauri mobile plugin**
   (recommended). Kotlin owns Android GATT and recording lifecycle; an
   established protocol implementation owns transport/crypto. Rust handles
   normalized records, summaries and the existing database. Meets the selected
   phone-only, direct-Bluetooth objective.
2. **Desktop BLE helper feeding the app**. Fastest reuse of the Python client,
   but requires a nearby running computer. Useful as a development control,
   not the proposed daily-use architecture.
3. **PWA-only Bluetooth**. Retain the PWA for imports and viewing records, but
   do not use a browser tab as the continuous Air recorder. It would also need
   the private Gattlink/DTLS/CoAP protocol, not just Web Bluetooth calls.

These are engineering recommendations; no new library compatibility or
owner approval is implied by this document.

## Protocol we actually have to implement

Evidence: the private research checkout's `ble/airclient.py`,
`ble/SUCCESS-gap1.md`, `decode-liveactivity.py`, and `LIMITATIONS.md`.
The success note proves an independent session, but explicitly records
sustained-polling instability. A short successful read is not evidence of
reliable all-day recording.

```text
Android BluetoothGatt (central/client)
  -> Gattlink reliable framing, flow control and retransmission
  -> IPv4 / UDP 5684 carried inside the BLE link
  -> DTLS 1.2 PSK SERVER on phone (Air is DTLS client)
  -> CoAP GET /liveactivity
  -> protobuf live reading
```

The DTLS server is internal to the link. Do not expose a LAN server.
The tracker is not a standard heart-rate-only peripheral.

Observed GATT endpoints:

| Role | UUID / action |
| --- | --- |
| Gattlink service | `abbaff00-e56a-484c-b832-8b17cf6cbfe8` |
| Phone to tracker | `abbaff01-e56a-484c-b832-8b17cf6cbfe8`, write without response |
| Tracker to phone | `abbaff02-e56a-484c-b832-8b17cf6cbfe8`, notifications |
| Cache handshake | Read `ac2f0145-8182-4be5-91e0-2992e6b40ebb`; write `00` to `ac2f2845-8182-4be5-91e0-2992e6b40ebb` with response |

Subscribe through CCCD, serialize GATT operations, then negotiate/reset
Gattlink and wait for DTLS authentication before reporting Connected.
Use conservative ATT payload sizes until negotiated MTU/write limits are
known. The research client uses 20-byte payload fragments. Android connection
callbacks alone do not prove protocol readiness.

Live protobuf mapping from research:

| Field | Interpretation | Ingestion |
| --- | --- | --- |
| 1 | Unix seconds | Convert to UTC milliseconds; retain receive time and clock-quality metadata. |
| 2 | Steps counter | Raw cumulative reading; reset policy remains provisional. |
| 3 | Distance, millimetres | Divide by 1,000 to store canonical metres (`Unit::Meters`); retain raw field. |
| 4 | Calories candidate | Provisional cumulative reading; calorie scope unspecified. |
| 7 | Heart rate, bpm | Instant reading; validate missing/sentinel values and quality before use. |
| 8 | HR quality flag | Preserve raw flag; do not invent a percentage confidence. |
| 5,6,9,10 | Unknown | Do not assign health meanings or show as validated metrics. |

The phone must handle device-initiated CoAP traffic to avoid starving live
polling. The Python prototype acknowledges requests with 2.04; do not copy
its blanket success response as production semantics. Catalogue observed
routes, acknowledge message delivery correctly, and implement bounded,
route-specific responses backed by evidence. Never send `sync/response`,
sync-complete messages or history-deletion commands in live-only mode.
If live polling cannot coexist with an uncompleted history sync, report the
blocker rather than pretending recording is healthy.

## Protocol implementation decision gate

Golden Gate supplies Gattlink, IP/UDP, DTLS-PSK server support and CoAP.
Inspected upstream revision: `b8a6f2552e12aaf92c1549dc88a41c9b7e73f1aa`.
Its Kotlin `Stack` has `isNode=false` hub mode and
`DtlsSocketNetifGattlink` (`DSNG`); the TLS key resolver supports the server
role. This matches the required shape, not yet proven Air interoperability.

However, upstream is archived. Its Android build uses AGP 4.1, JCenter,
RxJava 2 and the RxFitbitGatt module. Do not import its complete Android
application or obsolete build configuration into this repository.

First implementation milestone: test a minimal current-NDK build of the
required Golden Gate C components with a small application-owned JNI bridge
and current Android GATT transport. Verify Air-compatible DTLS cipher,
address configuration (`169.254.0.2` host, `.3` Air, port 5684), key length,
identity resolver, fragmentation, retransmission, and CoAP interoperability.
Pin upstream/submodule revisions, preserve Apache-2.0 notices, and review
the bundled TLS dependency before including it in a distributable APK.

Go/no-go: obtain a reproducible Android library build and a real authenticated
`/liveactivity` response before implementing the full recording UX. If this
fails, document the actual failure and evaluate a maintained DTLS-PSK engine
with a tested Gattlink implementation. Do not hand-write cryptography or
assume Android's ordinary TLS socket supports DTLS-PSK server mode.

## Runtime ownership and persistence

```text
Kotlin connectedDevice foreground service
  -> serialized GATT + protocol worker
  -> bounded, app-private durable raw-reading spool (SQLite)
  -> native Rust ingestion/reconciliation worker
  -> airspace-personal.sqlite + durable session/gap state
  -> best-effort UI events / snapshot query
```

The service owns collection; the webview only displays it. Commit each decoded
reading envelope to the spool before emitting it. Envelopes contain a stable
session/sequence identifier, device and receive timestamps, local UTC offset,
and decoded raw fields; never PSKs or plaintext network capture logs.

Rust drains the spool and validates/normalizes readings. One transaction in
the PERSONAL database commits measurements with deterministic IDs, session
progress, ingestion high-water marks and durable dirty-day markers. Only
after that transaction commits does the worker acknowledge spool rows. The
current `db::put_measurements` transaction alone is insufficient. A crash
between commit and acknowledgement causes safe replay. Dirty-day markers
survive until summaries are successfully recomputed and their markers cleared
atomically, so a crash after spool acknowledgement cannot leave stale
summaries permanently. Verify crash/replay on both sides of each boundary.
If the Tauri runtime is unavailable, the service keeps its spool; reconcile
on restart. Bound retention and stop with an explicit storage error before
discarding uncommitted data. Never claim an event-only pipeline is durable.

Latest-value signals may update once a second. Recompute summaries in batches
and reload dirty days on foreground/resume; no full-dataset scan per reading.
Persist session state and the last committed receive time so an unclean
restart creates an explicit gap. Process death and force-stop cannot promise
continuous recording or automatic recovery. Require a user-visible restart
when Android requires it.

Demo selection must never redirect a recording into the demo database. Stop
recording before full deletion; coordinate the spool, native database and
credential deletion so queued samples cannot resurrect erased data.

## Credentials and personal setup

Use the Android system file picker to import a versioned credential document
created locally from the already recovered key. Parse it on the native side;
do not put the key in JavaScript, route parameters, QR screenshots, sample
files, logs, Git or health backups. Bind credentials to the selected device.
Reject malformed base64, unsupported key lengths and invalid identity fields.

Research identity format is `MD-%08X-%08X`, constructed from key ID and
expiration in Unix seconds (source expiration is milliseconds). Match the
expected identity. Do not port the Python store's fallback that hands out
the primary key for any identity. Support only explicitly provisioned
identities until rotation is researched.

Store credentials through shared native Keystore storage usable by the
service without an Activity; refactor the vault's crypto helper rather than
creating a second plaintext preference store. Expose only configured/expiry
metadata to the interface. Expiration is a warning until enforcement is
tested; do not classify every handshake timeout as expired authorization.

Setup: import credentials -> grant discovery/connect permissions -> scan and
choose the Air -> start while the app is visible -> immediately promote the
service with a Connecting notification -> authenticate -> first valid reading
-> update the notification to Recording. Foreground promotion must meet the
platform deadline; never wait for a radio handshake to show the notification.
Another app holding the Air connection
may prevent access; explain that the official app must release its connection.
Do not ask to turn off Bluetooth on the recording Pixel itself.

## Android lifecycle and UI integration

Add Android 12+ scan/connect permissions, Android 8-11 scan/location handling
if supporting the existing minSdk 26, foreground-service permissions and the
`connectedDevice` service type. Verify current targetSdk 37 requirements
against official documentation. Request notification permission where
applicable, but distinguish denial from BLE permission denial; notification
denial is not by itself proof a foreground service cannot run.

Start from explicit user action. Show a recording notification with Stop.
Implement bounded reconnection/backoff and serialized cancellation/cleanup.
Do not add boot auto-start or unlimited scanning in this first iteration.

Proposed native command surface:
`live_status`, `live_scan`, `live_cancel_scan`, `live_import_credentials`,
`live_start`, `live_stop`, `live_forget_device`, `live_reconcile`.
Credential import accepts a native picker result, not secret-bearing UI state.
Use one explicit Start action that returns recording intent; Connected is
reported only after protocol authentication. Keep protocol-ready, recording,
latest-sample freshness and storage health separate.

UI subscribes to sanitized events and queries a snapshot on launch/resume.
Snapshots include sequence/revision so event replay cannot regress state.
Disconnect UI listeners on teardown. Android service continues without them.
PWA stays import/view-only; demo remains simulation; no fake success buttons.

## Historical bootstrap

Add an offline converter for the saved research readback (`results` wrapper),
using the mappings in the private checkout's `web/server.py` and `web/metrics.py`
as evidence. Output `airspace.import` v1, which the app already previews and
deduplicates. Process files locally; do not commit owner health data.

Convert each available metric explicitly, preserving sleep intervals, HRV
method, temperature deviation, calorie scope and timezone assumptions. Reject
ambiguous or unsupported structures with a report. Do not treat the dashboard's
flattened explorer fields or BLE capture as historical samples automatically.
Keep import coverage and per-metric dates visible. One import is not ongoing
Google synchronization and contains only the records actually saved.

Before merging imported current-day data with live counters, add tests for:
first live counter at noon, tracker reset, midnight, interrupted recording,
import ending before a live interval, and a daily-total snapshot captured
before day-end. Current whole-day daily-total priority can hide later live
activity, and a counter delta across a radio gap must not become a fabricated
minute-by-minute trace. Retain a counter snapshot separately from observed
increments and mark unrecoverable temporal distribution as unknown.

## Build sequence and acceptance gates

1. **Protocol/build proof:** minimal native stack, strict PSK resolver, captured
   non-personal protocol fixtures; one real response on Pixel. No production
   credential fixture in Git. Stop here if stack compatibility is unresolved.
2. **Durable recorder:** foreground service, native credential import, spool,
   sessions/gaps, idempotent ingestion and Stop. Test with webview absent.
3. **App wiring:** real Android controls, state/snapshot bridge, live tiles,
   summary invalidation, explicit PWA limitations and unchanged demo isolation.
4. **Historical import:** converter, sanitized fixtures for all available
   metrics, timezone/overlap tests and idempotent import preview.
5. **Device validation:** release/debug APK build, sustained polling, screen-off
   recording, range loss/reconnect, Bluetooth off/on, permission revocation,
   storage error, process death, stop/delete and midnight. Compare counters
   against the device/official data before removing provisional labels.

Minimum hardware gate before describing recording as usable: 30 minutes of
timestamped live readings, including screen-off and disconnect/reconnect,
with no silent gap or demo writes. Overnight and battery-cost testing follow;
the first response does not satisfy this gate. Record actual results and
unverified cases; never report simulator tests as real-device proof.

## Proposed file map

- Create `crates/tauri-plugin-air/`: Rust mobile bridge, Kotlin service/GATT/
  credential import, minimal JNI protocol wrapper, manifest and permissions.
- Create `crates/airspace-core/src/live_ingest.rs`: validated raw-envelope to
  `Measurement` conversion and deterministic IDs, without I/O.
- Modify `crates/airspace-core/src/bluetooth.rs`: snapshots, recording/error
  state and stable event envelope.
- Modify `crates/airspace-core/src/aggregate.rs`: only targeted counter/coverage
  corrections demonstrated by the integration tests.
- Modify `crates/tauri-plugin-vault/android/`: share context-based native secret
  storage with the recording service, keeping retrieval native-only.
- Create `src-tauri/src/live.rs`; modify `db.rs` and `lib.rs`: native ingestion,
  migrations, reconciliation, lifecycle and command registration.
- Modify `app/src/platform.rs`, `state.rs`, `screens/profile.rs`,
  `screens/today.rs`: event/snapshot bridge, controls and live signals.
- Create `tools/convert_research_history.py` with synthetic fixture tests;
  document invocation without hard-coded private paths.
- Update workspace manifests, Android configuration, capabilities and existing
  Bluetooth guide alongside the implementation that needs each change.

## Primary references

- [Tauri mobile plugins](https://v2.tauri.app/develop/plugins/develop-mobile/)
- [Android background BLE](https://developer.android.com/develop/connectivity/bluetooth/ble/background)
- [Android BLE permissions](https://developer.android.com/develop/connectivity/bluetooth/bt-permissions)
- [Foreground service types](https://developer.android.com/develop/background-work/services/fgs/service-types#connected-device)
- [Golden Gate source](https://github.com/Google-Health-API/golden-gate/tree/b8a6f2552e12aaf92c1549dc88a41c9b7e73f1aa)

Open decisions: exact native stack dependency after the build proof; verified
counter-reset policy; actual Android 17 lifecycle behavior and credential
renewal. None is answered by the existing UI or a successful public API login.
