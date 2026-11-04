# Bluetooth integration guide (future native Android service)

This build contains **no Bluetooth code and no Fitbit protocol**. It defines
the contract and the UI states a future implementation plugs into.

For the repository-specific connection design, protocol evidence, native
service/persistence architecture and staged device-validation gates, see
[Direct Fitbit Air recording design](superpowers/specs/2026-11-04-direct-bluetooth-design.md).
That document is a proposal; it does not enable Bluetooth in this build.

## What is known, and what isn't

* A separate client demonstrated live **heart rate, steps, distance and
  calories** over Bluetooth. Android behaviour and field semantics (units,
  whether steps are a cumulative counter, when it resets, device vs phone
  timestamps) are **unvalidated**.
* Historical records were obtained through Google-backed processing. Fully
  offline decryption of the tracker's encrypted history is **unsolved**.
* Device credential renewal is **unresolved**.
* Sleep stages, SpO₂, HRV, respiratory rate and skin temperature must **not**
  be inferred from the live stream.
* Connections drop; gaps must be recorded, not filled.

## Contract (`crates/airspace-core/src/bluetooth.rs`)

* `ConnectionState`: `Unavailable`, `PermissionDenied`, `BluetoothOff`,
  `NotPaired`, `Disconnected`, `Scanning`, `Connecting`, `Connected`,
  `Reconnecting`, `OutOfRange`, `AuthorizationExpired`.
* `DeviceInfo` (battery only when reported), `LiveSample` (metric, kind,
  timestamp, value, `provisional`, `time_basis`), `RecordingSession`,
  `RecordingGap` (with reason), `LiveEvent` stream, `Support` per metric
  (`live_capabilities()`), and `freshness()` for Live / Recent / Stale /
  Historical labels.
* `LiveProvider` trait: `state`, `device`, `start`, `stop`. `start` must fail
  with a real state, never pretend.

## Recommended Android design

1. **Foreground service** (`connectedDevice` type) owns the GATT connection,
   so recording survives the UI being backgrounded. Do not rely on Web
   Bluetooth or the WebView for background work.
2. **Permissions**: `BLUETOOTH_SCAN` (with `neverForLocation`),
   `BLUETOOTH_CONNECT`, `FOREGROUND_SERVICE_CONNECTED_DEVICE`,
   `POST_NOTIFICATIONS`. Map denial to `PermissionDenied { can_ask_again }`.
3. **Expose it to Rust** as a Tauri mobile plugin (same pattern as
   `crates/tauri-plugin-vault`): Kotlin `@TauriPlugin` class plus a Rust
   wrapper implementing `LiveProvider`, emitting `LiveEvent`s to the webview
   with `app.emit("live-event", …)`.
4. **Persist** each sample as a `Measurement` with `origin: bluetooth_live`,
   `status: measured`, `device` set, `provisional: true` for steps, distance
   and calories until validated. Write in small batches (e.g. every 30 s)
   through the same SQLite repository, then call the existing summary
   recomputation for the affected days.
5. **Counters**: store raw counter readings as `cumulative_counter`; never
   convert to increments at ingest. `aggregate.rs` already differences them,
   handles resets and avoids double counting with imported history.
6. **Gaps**: on disconnect, record a `RecordingGap` with its reason; on
   reconnect emit `Reconnected`. Never interpolate.
7. **Timestamps**: prefer the device clock when the protocol provides it
   (`TimeBasis::Device`), store UTC plus the phone's current offset.
8. **Credentials**: keep device credentials in the vault plugin; surface
   expiry as `AuthorizationExpired` and stop recording; never delete data.

## UI hooks already present

* Profile → Device shows the connection state, capability table and previews
  of every state.
* Today's status strip and heart-rate tile distinguish connected vs
  disconnected, recording vs not, live vs last reading, history vs Bluetooth,
  and fresh vs stale, using `freshness()`.
* Live values for a single metric update only the components that read them.

## Validation checklist before enabling

- [ ] Confirm units and semantics for each characteristic against reference data.
- [ ] Confirm counter reset behaviour (midnight? reboot?) and set `Detail::Counter { resets_daily }`.
- [ ] Measure clock drift between device and phone.
- [ ] Compare a full day of live recording with the imported history for the same day.
- [ ] Only then clear `provisional` for the validated metrics.
