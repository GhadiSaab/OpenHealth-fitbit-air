# Backup and export formats

## JSON backup (`airspace.backup`, version 1)

Created from Profile → Data → Export full backup. One file per database
(personal or demo).

```json
{
  "format": "airspace.backup",
  "version": 1,
  "created_at": 1793000000000,
  "app_version": "0.1.0",
  "database": "personal",
  "settings": { "theme": "system", "units": "metric", "timezone": "Europe/Berlin", "step_goal": 8000, … },
  "ai": { "provider": "anthropic", "endpoint": "https://api.anthropic.com", "model": "claude-opus-5-5" },
  "measurements": [ { "id": "imp-…", "metric": "steps", "kind": "interval", "start": 0, "end": 0,
                      "utc_offset_min": 120, "value": 912, "unit": "steps",
                      "prov": { "origin": "history_import", "status": "imported", "source_name": "…",
                                "batch_id": "…", "ingested_at": 0, "provisional": false, "demo": false },
                      "superseded_by": null } ],
  "journal": [ { "id": "j-2026-10-01", "day": "2026-10-01", "note": "…", "tags": ["exercise"], … } ],
  "batches": [ { "id": "imp-…", "file_name": "…", "added": 350, "duplicates": 0, "conflicts": 0, … } ]
}
```

* Times are UTC milliseconds; `utc_offset_min` preserves the original offset.
* Measurement fields are defined by `airspace_core::model::Measurement`
  (serde, snake_case enums). Derived daily summaries are **not** included; they
  are recalculated after restore.
* **Never included**: API keys or any credential. `Settings` and `AiConfig`
  have no field that could hold one, and a test asserts the serialized backup
  contains no key material.
* Restore validates `format` and `version`, refuses newer versions, refuses
  demo backups into the personal database (and vice versa), and offers
  **Merge** (skip existing ids) or **Replace**.
* Future versions add migration steps in `backup::parse_backup`.

## CSV export (per metric)

Columns:

| Column | Meaning |
|---|---|
| `id` | Airspace content id |
| `metric`, `kind`, `unit` | as in the model (snake_case, e.g. `cumulative_counter`) |
| `start_utc`, `end_utc` | ISO 8601 UTC |
| `local_day` | day the record is counted on |
| `utc_offset_min` | original offset, when known |
| `value` | canonical unit |
| `origin`, `status` | provenance enums, e.g. `history_import`, `bluetooth_live`; `imported`, `measured` |
| `source`, `device`, `original_id`, `batch_id`, `ingested_utc` | where and when it entered Airspace |
| `provisional`, `confidence`, `coverage`, `calc_version`, `superseded_by` | quality |
| `detail` | metric-specific JSON (sleep stages, HRV method, zone bounds, …) |

Text that starts with `=`, `+`, `-` or `@` is prefixed with `'` to prevent
spreadsheet formula injection.
