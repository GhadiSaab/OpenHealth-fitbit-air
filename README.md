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

## Screenshots

All screenshots come from the PWA build running the deterministic demo dataset (regenerate them with
`node scripts/screenshots.cjs`). The AI answer comes from a local mock provider, not a real model.

<table>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/01-onboarding.png" width="250" alt="Onboarding: demo, import, or set up later"><br><sub>Onboarding: demo, import, or set up later</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/02-today.png" width="250" alt="Today: summary, steps goal, heart rate with freshness, sleep"><br><sub>Today: summary, steps goal, heart rate with freshness, sleep</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/03-today-trends.png" width="250" alt="Today: distance, calories, trends, favorites"><br><sub>Today: distance, calories, trends, favorites</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/04-status-sheet.png" width="250" alt="Connection and freshness explained on tap"><br><sub>Connection and freshness explained on tap</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/05-explore.png" width="250" alt="Explore: all ten metric groups"><br><sub>Explore: all ten metric groups</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/06-heart-rate-day.png" width="250" alt="Intraday heart rate with min–max band and recording gap"><br><sub>Intraday heart rate with min–max band and recording gap</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/07-steps-month.png" width="250" alt="Steps month: goal line, 7-day average, no-data markers"><br><sub>Steps month: goal line, 7-day average, no-data markers</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/08-steps-calendar.png" width="250" alt="Steps calendar history and weekly consistency"><br><sub>Steps calendar history and weekly consistency</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/09-sleep-day.png" width="250" alt="Sleep stage timeline with measured vs calculated totals"><br><sub>Sleep stage timeline with measured vs calculated totals</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/10-hrv-month.png" width="250" alt="HRV with the measurement method preserved"><br><sub>HRV with the measurement method preserved</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/11-skin-temperature.png" width="250" alt="Skin temperature as a deviation, never core temperature"><br><sub>Skin temperature as a deviation, never core temperature</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/12-provenance-sheet.png" width="250" alt="Provenance on tap, down to each stored record"><br><sub>Provenance on tap, down to each stored record</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/13-journal.png" width="250" alt="Journal: calendar, notes and tags"><br><sub>Journal: calendar, notes and tags</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/14-journal-associations.png" width="250" alt="Goals, opt-in reminders, cautious tag comparisons"><br><sub>Goals, opt-in reminders, cautious tag comparisons</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/15-coach-insights.png" width="250" alt="Local insights with windows and coverage (no AI)"><br><sub>Local insights with windows and coverage (no AI)</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/16-ai-settings.png" width="250" alt="Bring-your-own-key AI provider settings"><br><sub>Bring-your-own-key AI provider settings</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/17-ai-consent.png" width="250" alt="Consent: dates, per-metric toggles, exact data shown"><br><sub>Consent: dates, per-metric toggles, exact data shown</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/18-ai-answer.png" width="250" alt="AI answers are labelled and kept distinct"><br><sub>AI answers are labelled and kept distinct</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/19-profile.png" width="250" alt="Profile and device entry"><br><sub>Profile and device entry</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/20-device.png" width="250" alt="Device: honest Not connected status, prepared states"><br><sub>Device: honest Not connected status, prepared states</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/21-data.png" width="250" alt="Data: demo vs personal, import, export, delete"><br><sub>Data: demo vs personal, import, export, delete</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/22-import-preview.png" width="250" alt="Import preview with counts, rejections, duplicates"><br><sub>Import preview with counts, rejections, duplicates</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/23-settings.png" width="250" alt="Settings: theme, units, time zone, motion"><br><sub>Settings: theme, units, time zone, motion</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/24-today-dark.png" width="250" alt="Dark theme"><br><sub>Dark theme</sub></td></tr>
<tr><td align="center" valign="top" width="33%"><img src="docs/screenshots/25-heart-rate-dark.png" width="250" alt="Dark theme chart"><br><sub>Dark theme chart</sub></td><td align="center" valign="top" width="33%"><img src="docs/screenshots/28-empty.png" width="250" alt="Usable with a completely empty database"><br><sub>Usable with a completely empty database</sub></td></tr>
</table>

**Desktop (responsive layout with a side rail)**

<p align="center">
<img src="docs/screenshots/26-desktop-dark.png" width="820" alt="Today on desktop, dark theme"><br>
<img src="docs/screenshots/27-desktop-steps.png" width="820" alt="Steps month view on desktop, light theme">
</p>

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

* **Rust tests** (`cargo test -p airspace-core -p airspace-tauri`): 53 pass. They cover:
  * aggregation, cumulative counters and resets, and overlap between imported history and live data;
  * daylight-saving days and overnight sleep;
  * import validation, duplicates and conflicts;
  * backup secrecy and CSV export;
  * AI request shapes and endpoint binding;
  * SQLite migrations and persistence after reopening;
  * a year of per-minute heart rate (about 525,000 samples), which summarizes in about 130 ms in release.
* **Browser test** (`scripts/e2e-smoke.cjs`, 33 checks) drives the PWA in headless Chromium. It passes, covering:
  * onboarding, offline reload after the first visit, an empty database, the demo, and persistence across reload;
  * charts with keyboard inspection and table view, and the journal;
  * Coach without AI, and the full AI flow against a local mock provider: connection test, consent, an excluded metric
    staying out, compact summaries only, the key never stored or put in a URL, and the answer labelled;
  * journal text surviving a tab switch, backup export with no key in it, import preview with rejected records, and
    re-import detected as duplicates;
  * dark theme, and no console errors.
* **The Android APK was not built or verified in this environment**, because the sandbox's network policy blocks the
  Android SDK download host.
  * The Android Studio project is committed in `src-tauri/gen/android`.
  * All Android-only Rust code type-checks for `aarch64-linux-android` with no errors or warnings, including reminders,
    the Keystore vault plugin and the mobile entry point. `scripts/android-typecheck.sh` does this without the NDK.
  * Not built here: the Kotlin `VaultPlugin`, the Gradle project, and the APK itself. Nothing has run on a device.
  * `ci/github-workflows/android.yml` builds the debug APK on GitHub's runners; see below.

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

Requirements: JDK 17, Android SDK (platform 37), NDK 27 (`NDK_HOME` set), Rust Android targets, and the Tauri CLI.
The Android Studio project is committed in `src-tauri/gen/android` (generated with `cargo tauri android init`, with
launcher icons, backup exclusions and no TV launcher).

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

The APK is written under `src-tauri/gen/android/app/build/outputs/apk/`. `android-prepare.sh` refreshes launcher icons
and checks that Android backup stays disabled (cloud backup and device transfer are excluded via
`res/xml/data_extraction_rules.xml`), so health data stays on the device. To run on a connected phone: `cargo tauri android dev`.

### CI workflows

`ci/github-workflows/` contains two GitHub Actions workflows: tests plus PWA plus browser test, and the debug APK
build. They were not pushed to `.github/workflows/` because the automation token lacked the `workflow` scope.
Enable them with:

```bash
mkdir -p .github/workflows && git mv ci/github-workflows/*.yml .github/workflows/ && git commit -m "Enable CI workflows" && git push
```

## Privacy and security

* No accounts, analytics, ads or tracking SDKs. No network requests except the ones you trigger to your AI provider.
* AI keys: Android stores them encrypted with an Android Keystore key, binds them to the endpoint they were entered
  for, and uses them only in native code. In the browser they live in memory only. Keys never appear in URLs, logs, `localStorage`, backups or the service worker cache.
* Before any AI request you see the exact dates, metrics and JSON to be shared; only daily summaries are sent.
* Android cloud backup is disabled for the app; use Airspace's own backup export.

## Documentation

* [Architecture and data rules](docs/ARCHITECTURE.md)
* [Import schema and adapter boundary](docs/IMPORT_SCHEMA.md)
* [Backup and CSV formats](docs/BACKUP_FORMAT.md)
* [Bluetooth integration guide](docs/BLUETOOTH_INTEGRATION.md)
* [Design system](docs/DESIGN.md)

Airspace is not a medical device and does not diagnose or treat any condition.
