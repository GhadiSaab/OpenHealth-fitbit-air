# Architecture

```
┌──────────────────────────── app/ (Leptos → WebAssembly) ────────────────────────────┐
│ screens/  Today · Explore · MetricDetail · Journal · Coach · Profile/Data/Import     │
│ ui/       charts (SVG) · widgets · icons · format                                    │
│ state.rs  signals + data pipeline (ingest → recompute summaries)                     │
│ repo.rs   repository façade ──┬── idb.rs  (IndexedDB, PWA)                          │
│                               └── Tauri commands (SQLite, Android)                   │
└──────────────────────────────────────┬───────────────────────────────────────────────┘
                                       │ uses
┌──────────────────────── crates/airspace-core (pure Rust, no I/O) ───────────────────┐
│ model · time · dedupe · aggregate · series · insights · journal · import · backup    │
│ demo · ai · bluetooth (interfaces) · providers · catalog · settings                  │
└──────────────────────────────────────────────────────────────────────────────────────┘
┌──────────────── src-tauri (native) ───────────────┐  ┌── crates/tauri-plugin-vault ──┐
│ db.rs SQLite + migrations · ai_request (ureq)     │  │ Android Keystore AES-GCM       │
│ export_file (save dialog) · reminders_set         │──│ memory-only on desktop         │
└───────────────────────────────────────────────────┘  └────────────────────────────────┘
```

Data processing never lives in UI components: screens read derived summaries
and call core functions; the core has no knowledge of storage or rendering.

## Providers

| Boundary | Implementation in this build |
|---|---|
| `DemoDataProvider` | `airspace_core::demo::generate` → demo database |
| `HistoryImportProvider` | `airspace_core::import::{ImportAdapter, parse_file, preview}` |
| `BluetoothLiveProvider` | `airspace_core::bluetooth::LiveProvider`; only `NotConnectedLive` |
| `OptionalHistorySyncProvider` | `airspace_core::providers::NotConnectedHistorySync` |
| `AIProvider` | `airspace_core::ai` (Anthropic, OpenAI-compatible) + platform transport |

## Storage

Two databases that never mix: **personal** and **demo** (IndexedDB names
`airspace-personal` / `airspace-demo`; SQLite files `airspace-personal.sqlite`
/ `airspace-demo.sqlite` in app-private storage). Stores/tables:

| Store | Key | Content |
|---|---|---|
| `measurements` | `id` (+ index `metric,start`, `start`) | `Measurement` JSON |
| `summaries` | `metric|day` | derived `DaySummary` JSON |
| `journal` | `id` | `JournalEntry` JSON |
| `batches` | `id` | `ImportBatch` JSON |
| `meta` | `key` | demo device state, summary time zone, … |

Schema version `DB_SCHEMA_VERSION` (currently 1). IndexedDB runs ordered
steps in `onupgradeneeded`; SQLite runs ordered migrations tracked with
`PRAGMA user_version`, each in a transaction. Add a migration by appending a
step in both `app/src/idb.rs::migrate` and `src-tauri/src/db.rs::MIGRATIONS`.

UI preferences (route, selected day, settings, AI provider config without
key) are kept in `localStorage`.

## Measurements and provenance

Every `Measurement` carries metric, unit, kind (`instant`, `interval`,
`cumulative_counter`, `daily_total`, `session`), UTC start/end, the original
UTC offset when known, value, metric-specific detail, and `Provenance`:
origin (history import / Bluetooth / derived), status (measured / imported /
derived), source name, device, original id, import batch, ingestion time,
provisional flag, coverage, confidence, calculation version, demo flag.

IDs are deterministic content hashes, so re-importing a file yields
duplicates rather than copies. Conflicting values for the same slot are
never overwritten: "prefer incoming" stores the new record and marks the old
one `superseded_by`, keeping it in the database and backups.

## Time rules

* Instants are UTC milliseconds.
* A record's local day uses its recorded offset when present, otherwise the
  home time zone (Settings). Day boundaries follow the home zone, so DST days
  are 23 or 25 hours.
* Offset-less local timestamps that are ambiguous (DST fall-back) resolve to
  the earlier instant with an import warning; nonexistent ones are rejected.
* Sleep belongs to the day it **ends**.
* Changing the home zone recalculates all summaries.

## Aggregation rules (`aggregate.rs`)

* Missing data is `None`, never zero. Charts show an explicit "no data" mark
  and never draw lines across gaps.
* **Additive metrics** (steps, distance, calories): sources are ranked
  (imported daily total → imported intervals → live intervals → live counter
  → imported counter). Each contributes only for time not already covered by
  a higher-ranked source, so history and live recording combine without
  double counting.
* **Cumulative counters** are differenced; a decrease is a reset (the new
  reading counts from zero); implausible jumps are rejected. Readings are
  never summed.
* **HRV** uses one method per day (RMSSD preferred); others are counted and
  excluded, never averaged together.
* **Calories** report total vs active only when the source says so.
* Summaries carry `calc_version`; outdated ones are recomputed on load.

Intraday charts use `additive_increments`, which applies the same rules, so
hourly bars add up to the daily total. Large intraday series are bucketed
(min/max/avg); raw samples stay untouched in storage.

## Insights

`insights::generate` produces only comparisons with sufficient coverage
(e.g. ≥5 of 7 days on both sides), and always reports the window and
coverage. There is no readiness/sleep/recovery score.

## AI

`ai.rs` builds requests and parses responses; it contains no key. The
platform layer sends them: Android via the `ai_request` command (key read
from the vault in Rust, never visible to the webview, URL re-validated); the
browser via `fetch` with a memory-only key. The consent screen shows the
exact JSON (`build_context`) before sending. Journal notes are included only
on request and the system prompt instructs the model to treat them as data.

## Performance

* Fine-grained signals: a new reading updates only the components that read
  it, not the whole dashboard.
* Summaries (a few KB per month) are held in memory; raw samples are loaded
  per range on demand.
* Writes are batched (5 000 rows per transaction).
