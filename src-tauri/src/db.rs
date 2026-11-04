//! SQLite repository for the Android app.
//!
//! One file per database (`airspace-personal.sqlite`, `airspace-demo.sqlite`)
//! in the app's private data directory. Schema versions are tracked with
//! `PRAGMA user_version`; each migration runs once, in order, inside a
//! transaction. Rows hold the same JSON documents as the PWA's IndexedDB.

use airspace_core::Measurement;
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::Manager;

#[derive(Default)]
pub struct Dbs(Mutex<HashMap<String, Connection>>);

const MIGRATIONS: &[&str] = &[
    // v1
    "CREATE TABLE measurements (id TEXT PRIMARY KEY, metric TEXT NOT NULL, start INTEGER NOT NULL, json TEXT NOT NULL);
     CREATE INDEX measurements_metric_start ON measurements(metric, start);
     CREATE INDEX measurements_start ON measurements(start);
     CREATE TABLE summaries (key TEXT PRIMARY KEY, json TEXT NOT NULL);
     CREATE TABLE journal (key TEXT PRIMARY KEY, json TEXT NOT NULL);
     CREATE TABLE batches (key TEXT PRIMARY KEY, json TEXT NOT NULL);
     CREATE TABLE meta (key TEXT PRIMARY KEY, json TEXT NOT NULL);",
];

pub fn migrate(c: &mut Connection) -> rusqlite::Result<()> {
    let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(v as usize) {
        let tx = c.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

fn table(name: &str) -> Result<&'static str, String> {
    // Whitelist: table names can't be bound as SQL parameters.
    Ok(match name {
        "measurements" => "measurements",
        "summaries" => "summaries",
        "journal" => "journal",
        "batches" => "batches",
        "meta" => "meta",
        _ => return Err("unknown table".into()),
    })
}

fn db_name(db: &str) -> Result<&'static str, String> {
    match db {
        "personal" => Ok("personal"),
        "demo" => Ok("demo"),
        _ => Err("unknown database".into()),
    }
}

pub fn with<T>(app: &tauri::AppHandle, db: &str, f: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
    let name = db_name(db)?;
    let state = app.state::<Dbs>();
    let mut map = state.0.lock().map_err(|_| "database lock poisoned")?;
    if !map.contains_key(name) {
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut c = Connection::open(dir.join(format!("airspace-{name}.sqlite"))).map_err(|e| e.to_string())?;
        c.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
        migrate(&mut c).map_err(|e| format!("database migration failed: {e}"))?;
        map.insert(name.to_string(), c);
    }
    f(map.get_mut(name).unwrap())
}

pub fn put_measurements(c: &mut Connection, rows: &[String]) -> Result<(), String> {
    let tx = c.transaction().map_err(|e| e.to_string())?;
    {
        let mut st = tx.prepare_cached("INSERT OR REPLACE INTO measurements (id, metric, start, json) VALUES (?1, ?2, ?3, ?4)").map_err(|e| e.to_string())?;
        for r in rows {
            // Validate against the core model before storing.
            let m: Measurement = serde_json::from_str(r).map_err(|_| "invalid measurement".to_string())?;
            st.execute(params![m.id, m.metric.key(), m.start, r]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn measurements(c: &mut Connection, metric: Option<&str>, from: i64, to: i64) -> Result<Vec<String>, String> {
    let mut out = vec![];
    match metric {
        Some(m) => {
            let mut st = c.prepare_cached("SELECT json FROM measurements WHERE metric = ?1 AND start >= ?2 AND start < ?3 ORDER BY start").map_err(|e| e.to_string())?;
            let rows = st.query_map(params![m, from, to], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
            for r in rows {
                out.push(r.map_err(|e| e.to_string())?);
            }
        }
        None => {
            let mut st = c.prepare_cached("SELECT json FROM measurements WHERE start >= ?1 AND start < ?2 ORDER BY start").map_err(|e| e.to_string())?;
            let rows = st.query_map(params![from, to], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
            for r in rows {
                out.push(r.map_err(|e| e.to_string())?);
            }
        }
    }
    Ok(out)
}

pub fn all(c: &mut Connection, t: &str) -> Result<Vec<String>, String> {
    let t = table(t)?;
    let mut st = c.prepare(&format!("SELECT json FROM {t}")).map_err(|e| e.to_string())?;
    let rows = st.query_map([], |r| r.get::<_, String>(0)).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn put_keyed(c: &mut Connection, t: &str, rows: &[(String, String)]) -> Result<(), String> {
    let t = table(t)?;
    if t == "measurements" {
        return Err("use put_measurements".into());
    }
    let tx = c.transaction().map_err(|e| e.to_string())?;
    {
        let mut st = tx.prepare(&format!("INSERT OR REPLACE INTO {t} (key, json) VALUES (?1, ?2)")).map_err(|e| e.to_string())?;
        for (k, v) in rows {
            st.execute(params![k, v]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn delete_keyed(c: &mut Connection, t: &str, keys: &[String]) -> Result<(), String> {
    let t = table(t)?;
    let col = if t == "measurements" { "id" } else { "key" };
    let tx = c.transaction().map_err(|e| e.to_string())?;
    {
        let mut st = tx.prepare(&format!("DELETE FROM {t} WHERE {col} = ?1")).map_err(|e| e.to_string())?;
        for k in keys {
            st.execute(params![k]).map_err(|e| e.to_string())?;
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn get_keyed(c: &mut Connection, t: &str, key: &str) -> Result<Option<String>, String> {
    let t = table(t)?;
    let mut st = c.prepare(&format!("SELECT json FROM {t} WHERE key = ?1")).map_err(|e| e.to_string())?;
    let mut rows = st.query(params![key]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(r) => Ok(Some(r.get(0).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

pub fn clear(c: &mut Connection) -> Result<(), String> {
    c.execute_batch("DELETE FROM measurements; DELETE FROM summaries; DELETE FROM journal; DELETE FROM batches; DELETE FROM meta; VACUUM;").map_err(|e| e.to_string())
}

pub fn count(c: &mut Connection) -> Result<u32, String> {
    c.query_row("SELECT COUNT(*) FROM measurements", [], |r| r.get::<_, u32>(0)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use airspace_core::demo;

    fn mem() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        c
    }

    #[test]
    fn migrations_are_idempotent_and_persist_round_trip() {
        let dir = std::env::temp_dir().join(format!("airspace-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.sqlite");
        let _ = std::fs::remove_file(&path);
        let tz = airspace_core::time::parse_tz("UTC");
        let end = airspace_core::time::parse_day("2026-10-30").unwrap();
        let ds = demo::generate(end, &tz, airspace_core::time::day_start(end, &tz) + 43_200_000);
        let rows: Vec<String> = ds.measurements.iter().take(2000).map(|m| serde_json::to_string(m).unwrap()).collect();
        {
            let mut c = Connection::open(&path).unwrap();
            migrate(&mut c).unwrap();
            put_measurements(&mut c, &rows).unwrap();
            put_keyed(&mut c, "meta", &[("k".into(), "v".into())]).unwrap();
        }
        // "Restart": reopen and migrate again.
        let mut c = Connection::open(&path).unwrap();
        migrate(&mut c).unwrap();
        assert_eq!(count(&mut c).unwrap(), 2000);
        assert_eq!(get_keyed(&mut c, "meta", "k").unwrap().as_deref(), Some("v"));
        // Re-inserting the same rows does not duplicate.
        put_measurements(&mut c, &rows).unwrap();
        assert_eq!(count(&mut c).unwrap(), 2000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_unknown_tables_and_invalid_rows() {
        let mut c = mem();
        assert!(all(&mut c, "sqlite_master").is_err());
        assert!(put_measurements(&mut c, &["{}".into()]).is_err());
    }
}
