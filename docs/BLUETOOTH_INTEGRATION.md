# Bluetooth integration guide

Native recording is implemented in `crates/tauri-plugin-air`, with durable ingestion
into the personal SQLite database. The browser PWA does not provide this BLE link.

## Using the Android APK

1. Pair the Air once with Google Health and provision an existing credential file.
   Independent account sign-in, initial pairing and key renewal remain unresolved.
2. Open Airspace's Device page, grant Nearby Devices and notification permissions,
   and import the credential file through the Android picker. Credentials stay in
   Android Keystore-encrypted native storage; backup/export excludes them.
3. Release the Air from Google Health before starting Airspace. Google Health can
   reconnect in the background; temporarily disabling its Nearby Devices permission
   may be necessary. Do not unpair or reset the tracker.
4. Press Start. Recording starts only after an authenticated private-key session
   receives readings. The foreground service can continue with the screen off.
   Stop ends recording; disabling Bluetooth requires turning it on and pressing
   Start again. Existing readings remain saved.

November 5 Pixel tests demonstrated private-key readings, screen-off operation,
Stop and an explicit restart after Bluetooth off/on. A long-duration run and
recovery from every tracker failure remain unverified. If the Air presents the
public BOOTSTRAP identity, recording rejects it. Briefly resuming Google Health
restored private-identity selection in one observed test; this is a workaround,
not a proven general recovery mechanism.

The following contract and design notes include earlier research assumptions;
see [RESUME.md](../RESUME.md) for the latest evidence.

## What is known, and what isn't

* A separate client demonstrated live **heart rate, steps, distance and
  calories** over Bluetooth. Some field semantics (units,
  counter resets and device clock interpretation) remain **partly unvalidated**.
  Heart rate, cumulative steps, distance and provisional calories are ingested.
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
