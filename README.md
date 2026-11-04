# Airspace

A private, local-first fitness companion for a Fitbit Air. Your data stays on
your device: no account, no subscription, no analytics, and no network
needed for anything except the optional AI coach.

Airspace is written entirely in **Rust**:

| Part | Technology | Where |
|---|---|---|
| Domain logic (models, provenance, aggregation, import, export, insights, AI adapters) | Rust library, no I/O | `crates/airspace-core` |
| User interface | [Leptos](https://leptos.dev) 0.8 compiled to WebAssembly | `app/` |
| Installable PWA | Same WebAssembly UI + service worker, IndexedDB storage | `app/dist` after `trunk build` |
| Android app | [Tauri 2](https://v2.tauri.app) shell, SQLite (`rusqlite`), native HTTPS (`ureq`) | `src-tauri/` |
| Secret storage | Small Tauri plugin: Android Keystore AES-GCM | `crates/tauri-plugin-vault` |

## What works, what's simulated, what's deferred

| Area | Status |
|---|---|
| Today, Explore (10 metric groups, day/week/month/year), Journal, Coach | **Implemented** |
| Local persistence: IndexedDB (PWA), SQLite (Android), versioned migrations | **Implemented** |
| History import (JSON/CSV) with preview, validation, duplicate & conflict handling | **Implemented** |
| Versioned JSON backup/restore, per-metric CSV export, full deletion | **Implemented** |
| Local insights (rolling averages, period comparisons, timing & activity consistency, baselines) | **Implemented** |
| Optional AI coach: Anthropic adapter + OpenAI-compatible adapter, consent screen, key handling | **Implemented** (requires your own key; not exercised against a live provider in this build) |
| Reminders (opt-in, Android only) | **Implemented** (Android path untested on a device) |
| Haptics on meaningful actions (Android) | **Implemented** (untested on a device) |
| 30-day demo dataset (every metric, missing day, charging gap, unstaged night, mixed HRV methods, disconnected live session) | **Simulated** — clearly marked, separate database |
| Bluetooth live recording | **Deferred** — typed interface and UI states only; reports "Not connected" |
| Google-backed history sync | **Deferred** — reports "Not connected" |
| Offline decryption of tracker history | **Not attempted** (unsolved; see Bluetooth guide) |

No feature pretends to work: unavailable adapters say what is missing.

## Verification status

* `cargo test -p airspace-core -p airspace-tauri`: 50 tests pass (aggregation, cumulative counters, overlap handling,
  DST and overnight sleep, import validation, duplicates and conflicts, backup secrecy, SQLite migrations and persistence).
* `scripts/e2e-smoke.cjs` drives the PWA build in headless Chromium and passes, covering: onboarding; empty database; demo;
  persistence across reload; charts with keyboard inspection and table view; journal; Coach without AI; API key not
  stored or exported; backup export; import preview with rejected records; re-import detected as duplicates; dark theme;
  no console errors.
* **The Android APK was not built or verified in this environment.** The Android SDK download host is blocked by the
  sandbox's network policy. The Android Rust code for the Keystore plugin type-checks for `aarch64-linux-android`; the
  rest is verified only on desktop targets. A GitHub Actions workflow that builds the debug APK is provided in
  `ci/github-workflows/android.yml` (see below).

## Build and run

Prerequisites: Rust stable (1.90+), `rustup target add wasm32-unknown-unknown`, and

```bash
cargo install trunk --version 0.22.1 --locked
```

```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

### Tests

```bash
cargo test -p airspace-core
```

`airspace-tauri` tests additionally need the Tauri Linux libraries (`libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libsoup-3.0-dev`):

```bash
cargo test -p airspace-tauri
```

### PWA

```bash
cd app && trunk serve
```

Open http://127.0.0.1:8080. For a production build, run `trunk build --release` in `app/`. The output in `app/dist` is a
static site; serve it over HTTPS to install it as an app. Browser smoke test (needs Node and Playwright with Chromium):

```bash
node scripts/e2e-smoke.cjs
```

### Android debug APK

Requirements: JDK 17, Android SDK (platform 36, build-tools 36), NDK 27 (`NDK_HOME` set), Rust Android targets, and
the Tauri CLI.

```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

```bash
cargo install tauri-cli --version 2.12.4 --locked
```

```bash
(cd app && trunk build --release) && scripts/android-prepare.sh
```

```bash
cd src-tauri && cargo tauri android build --debug --apk --target aarch64 x86_64
```

The APK is written under `src-tauri/gen/android/app/build/outputs/apk/`. `android-prepare.sh` generates the Android
project (`cargo tauri android init`), installs the app icons and disables Android cloud backup so health data stays on
the device. To run on a connected phone: `cargo tauri android dev`.

### CI workflows

`ci/github-workflows/` contains two GitHub Actions workflows: tests plus PWA plus browser test, and the debug APK
build. They were not pushed to `.github/workflows/` because the automation token lacked the `workflow` scope.
Enable them with:

```bash
mkdir -p .github/workflows && git mv ci/github-workflows/*.yml .github/workflows/ && git commit -m "Enable CI workflows" && git push
```

## Privacy and security

* No accounts, analytics, ads or tracking SDKs. No network requests except the ones you trigger to your AI provider.
* AI keys: Android stores them encrypted with an Android Keystore key and uses them only in native code. In the browser
  they live in memory only. Keys never appear in URLs, logs, `localStorage`, backups or the service worker cache.
* Before any AI request you see the exact dates, metrics and JSON to be shared; only daily summaries are sent.
* Android cloud backup is disabled for the app; use Airspace's own backup export.

## Documentation

* [Architecture and data rules](docs/ARCHITECTURE.md)
* [Import schema and adapter boundary](docs/IMPORT_SCHEMA.md)
* [Backup and CSV formats](docs/BACKUP_FORMAT.md)
* [Bluetooth integration guide](docs/BLUETOOTH_INTEGRATION.md)
* [Design system](docs/DESIGN.md)

Airspace is not a medical device and does not diagnose or treat any condition.
