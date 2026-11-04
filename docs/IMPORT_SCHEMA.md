# Import schema (version 1)

Airspace imports JSON or CSV in the format below. Existing exports, including
the earlier research exports, are **not** assumed to match it. Convert them
with a script, or add an adapter (see the end of this page).

Examples: `examples/sample-import.json`, `examples/sample-import.csv`.

## JSON

```json
{
  "schema": "airspace.import",
  "version": 1,
  "source": { "name": "My export", "device": "Fitbit Air" },
  "timezone": "Europe/Paris",
  "records": [ … ],
  "journal": [ { "day": "2026-10-01", "note": "Ran 5k", "tags": ["exercise"] } ]
}
```

* `timezone` (IANA) is used for timestamps without an offset; otherwise the user's home zone.
* Timestamps: RFC 3339 with offset (preferred), `YYYY-MM-DDTHH:MM[:SS]` local time, or epoch milliseconds.

### Record fields

| Field | Required | Notes |
|---|---|---|
| `metric` | yes | `heart_rate`, `steps`, `sleep`, `resting_heart_rate`, `heart_rate_zones`, `distance`, `calories`, `spo2`, `hrv`, `skin_temperature`, `respiratory_rate` |
| `kind` | no | `instant`, `interval`, `cumulative_counter`, `daily_total`, `session`. Inferred: sleep → session; `day` only → daily_total; `start`+`end` → interval; else instant |
| `time` | instant/counter | reading time |
| `start`, `end` | interval/session | `end` exclusive, must be after `start`, at most 24 h |
| `day` | daily_total | `YYYY-MM-DD` local day |
| `value` | yes (except sleep) | in `unit` |
| `unit` | no | defaults to the canonical unit; conversions below |
| `id` | no | the source's identifier (kept as provenance) |
| `confidence` | no | 0–3 |
| `provisional` | no | `true` if the field mapping is unvalidated |

Metric-specific fields:

| Metric | Fields |
|---|---|
| `hrv` | `method`: `rmssd` or `sdnn` (missing → "unspecified", kept separate) |
| `calories` | `scope`: `total` or `active` (missing → unspecified) |
| `heart_rate_zones` | `zone` (required): `below_zones`, `fat_burn`, `cardio`, `peak`; `lower_bpm`, `upper_bpm`; `zone_origin`: `imported` (default) or `calculated` |
| `spo2` | `min`, `max` |
| `respiratory_rate` | `stage`: `light`, `deep`, `rem` (omit for the overnight average) |
| `steps`, `distance` counters | `resets_daily`: `true` if the counter restarts at local midnight |
| `sleep` | `asleep_minutes` (else derived from stages), `main` (default `true`), `stages`: `[{stage, start, end}]` with `stage` in `awake`, `light`, `deep`, `rem`, `asleep`, `restless` |

### Units

| Metric | Canonical | Also accepted |
|---|---|---|
| heart rate, resting HR | `bpm` | |
| steps | `steps` | `count` |
| distance | `m` | `km`, `mi`, `ft` |
| calories | `kcal` | `kJ` |
| sleep, zones | `min` | `h`, `s` |
| SpO₂ | `%` | |
| HRV | `ms` | |
| skin temperature | `delta_c` | `delta_f`. Absolute temperatures (`°C`, `°F`) are **rejected** |
| respiratory rate | `br/min` | `breaths/min` |

### Validation

Records are rejected (shown as "Skipped" in the preview) for unknown metrics,
malformed or nonexistent timestamps, values outside plausible ranges (e.g.
heart rate 25–250 bpm, SpO₂ 50–100 %), timestamps in the future or before
2000, invalid units, and malformed stages. Warnings (record kept) cover
ambiguous DST times, overlapping stages, missing HRV method, and more than
300 steps per minute.

## CSV

The first line is a header using the same field names (any order). Sleep
stages can't be expressed in CSV. Unknown columns are ignored with a warning.

```csv
metric,kind,time,start,end,day,value,unit,method,scope
resting_heart_rate,,2026-10-25T07:00:00+02:00,,,,56,bpm,,
steps,daily_total,,,,2026-10-25,8450,,,
```

## Duplicates and conflicts

* **Duplicate**: same content id (metric, kind, time span, value, detail,
  source, original id). Skipped.
* **Conflict**: same metric/kind/time span/detail but a different value. You choose:
  keep existing (default), prefer the file (the original is kept and marked
  superseded), or keep both (daily totals still avoid double counting).
* Imported journal entries are added only for days without an entry.

## Adding an adapter

Implement `airspace_core::import::ImportAdapter`:

```rust
pub trait ImportAdapter {
    fn id(&self) -> &'static str;          // recorded with each batch
    fn name(&self) -> &'static str;
    fn detect(&self, file_name: &str, content: &str) -> bool;
    fn parse(&self, content: &str, ctx: &ImportContext) -> ParsedImport;
}
```

Map the source to `RawRecord`s and call `raw::convert_all` so the shared
validation applies, then register the adapter in `import::adapters()`. Do not
guess field semantics: if a source field's meaning is uncertain, set
`provisional: true` and document it.
