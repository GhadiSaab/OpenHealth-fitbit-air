//! Repository façade. The UI and the data pipeline call these functions;
//! they dispatch to IndexedDB (PWA) or SQLite through Tauri commands
//! (Android). Both backends store identical JSON documents.

use crate::idb;
use crate::platform::{invoke, is_tauri};
use airspace_core::aggregate::DaySummary;
use airspace_core::journal::JournalEntry;
use airspace_core::{DbKind, ImportBatch, Measurement, Metric};
use serde::de::DeserializeOwned;
use serde_json::json;

pub type R<T> = Result<T, String>;

fn db_name(k: DbKind) -> String {
    format!("airspace-{}", k.key())
}

fn parse_all<T: DeserializeOwned>(rows: Vec<String>) -> Vec<T> {
    // Unreadable rows are skipped rather than crashing the app; they remain stored.
    rows.iter().filter_map(|s| serde_json::from_str(s).ok()).collect()
}

async fn idb(k: DbKind) -> R<web_sys::IdbDatabase> {
    idb::open(&db_name(k)).await
}

pub async fn open(k: DbKind) -> R<()> {
    if is_tauri() {
        invoke::<()>("db_open", json!({"db": k.key()})).await
    } else {
        idb(k).await.map(|_| ())
    }
}

pub async fn put_measurements(k: DbKind, items: &[Measurement]) -> R<()> {
    if items.is_empty() {
        return Ok(());
    }
    if is_tauri() {
        for chunk in items.chunks(5000) {
            let rows: Vec<String> = chunk.iter().map(|m| serde_json::to_string(m).unwrap_or_default()).collect();
            invoke::<()>("db_put_measurements", json!({"db": k.key(), "rows": rows})).await?;
        }
        Ok(())
    } else {
        let db = idb(k).await?;
        for chunk in items.chunks(5000) {
            let rows = chunk
                .iter()
                .map(|m| idb::measurement_row(&m.id, m.metric.key(), m.start, serde_json::to_string(m).unwrap_or_default()))
                .collect();
            idb::put_rows(&db, idb::MEASUREMENTS, rows).await?;
        }
        Ok(())
    }
}

/// Measurements whose start is in `[from, to)`.
pub async fn measurements(k: DbKind, metric: Option<Metric>, from: i64, to: i64) -> R<Vec<Measurement>> {
    let rows: Vec<String> = if is_tauri() {
        invoke("db_measurements", json!({"db": k.key(), "metric": metric.map(|m| m.key()), "from": from, "to": to})).await?
    } else {
        idb::measurements_json(&idb(k).await?, metric.map(|m| m.key()), from, to).await?
    };
    Ok(parse_all(rows))
}

pub async fn all_measurements(k: DbKind) -> R<Vec<Measurement>> {
    let rows: Vec<String> = if is_tauri() {
        invoke("db_all", json!({"db": k.key(), "table": "measurements"})).await?
    } else {
        idb::all_json(&idb(k).await?, idb::MEASUREMENTS).await?
    };
    Ok(parse_all(rows))
}

pub async fn put_summaries(k: DbKind, items: &[DaySummary]) -> R<()> {
    if items.is_empty() {
        return Ok(());
    }
    if is_tauri() {
        let rows: Vec<(String, String)> = items.iter().map(|s| (s.key(), serde_json::to_string(s).unwrap_or_default())).collect();
        invoke("db_put_keyed", json!({"db": k.key(), "table": "summaries", "rows": rows})).await
    } else {
        let rows = items.iter().map(|s| idb::keyed_row("key", &s.key(), serde_json::to_string(s).unwrap_or_default())).collect();
        idb::put_rows(&idb(k).await?, idb::SUMMARIES, rows).await
    }
}

pub async fn delete_summaries(k: DbKind, keys: &[String]) -> R<()> {
    if is_tauri() {
        invoke("db_delete_keyed", json!({"db": k.key(), "table": "summaries", "keys": keys})).await
    } else {
        idb::delete_keys(&idb(k).await?, idb::SUMMARIES, keys).await
    }
}

pub async fn summaries(k: DbKind) -> R<Vec<DaySummary>> {
    all_keyed(k, "summaries", idb::SUMMARIES).await
}

async fn all_keyed<T: DeserializeOwned>(k: DbKind, table: &str, store: &str) -> R<Vec<T>> {
    let rows: Vec<String> = if is_tauri() {
        invoke("db_all", json!({"db": k.key(), "table": table})).await?
    } else {
        idb::all_json(&idb(k).await?, store).await?
    };
    Ok(parse_all(rows))
}

pub async fn put_journal(k: DbKind, items: &[JournalEntry]) -> R<()> {
    if items.is_empty() {
        return Ok(());
    }
    if is_tauri() {
        let rows: Vec<(String, String)> = items.iter().map(|e| (e.id.clone(), serde_json::to_string(e).unwrap_or_default())).collect();
        invoke("db_put_keyed", json!({"db": k.key(), "table": "journal", "rows": rows})).await
    } else {
        let rows = items.iter().map(|e| idb::keyed_row("id", &e.id, serde_json::to_string(e).unwrap_or_default())).collect();
        idb::put_rows(&idb(k).await?, idb::JOURNAL, rows).await
    }
}

pub async fn delete_journal(k: DbKind, id: &str) -> R<()> {
    if is_tauri() {
        invoke("db_delete_keyed", json!({"db": k.key(), "table": "journal", "keys": [id]})).await
    } else {
        idb::delete_keys(&idb(k).await?, idb::JOURNAL, &[id.to_string()]).await
    }
}

pub async fn journal(k: DbKind) -> R<Vec<JournalEntry>> {
    all_keyed(k, "journal", idb::JOURNAL).await
}

pub async fn put_batch(k: DbKind, b: &ImportBatch) -> R<()> {
    let s = serde_json::to_string(b).unwrap_or_default();
    if is_tauri() {
        invoke("db_put_keyed", json!({"db": k.key(), "table": "batches", "rows": [(b.id.clone(), s)]})).await
    } else {
        idb::put_rows(&idb(k).await?, idb::BATCHES, vec![idb::keyed_row("id", &b.id, s)]).await
    }
}

pub async fn batches(k: DbKind) -> R<Vec<ImportBatch>> {
    all_keyed(k, "batches", idb::BATCHES).await
}

pub async fn meta_get(k: DbKind, key: &str) -> R<Option<String>> {
    if is_tauri() {
        invoke("db_meta_get", json!({"db": k.key(), "key": key})).await
    } else {
        idb::get_json(&idb(k).await?, idb::META, key).await
    }
}

pub async fn meta_set(k: DbKind, key: &str, value: &str) -> R<()> {
    if is_tauri() {
        invoke("db_put_keyed", json!({"db": k.key(), "table": "meta", "rows": [(key, value)]})).await
    } else {
        idb::put_rows(&idb(k).await?, idb::META, vec![idb::keyed_row("key", key, value.to_string())]).await
    }
}

pub async fn clear(k: DbKind) -> R<()> {
    if is_tauri() {
        invoke("db_clear", json!({"db": k.key()})).await
    } else {
        idb::clear_all(&idb(k).await?).await
    }
}

pub async fn count(k: DbKind) -> R<u32> {
    if is_tauri() {
        invoke("db_count", json!({"db": k.key()})).await
    } else {
        idb::count(&idb(k).await?, idb::MEASUREMENTS).await
    }
}
