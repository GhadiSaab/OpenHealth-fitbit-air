//! Native ingestion runs without the WebView. Acknowledgement follows a durable commit.
use crate::db;
use airspace_core::{
    Measurement, Metric,
    aggregate::{self, SummaryContext},
    live_ingest::{self, RawLive},
    time,
};
use rusqlite::params;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_air::AirExt;
pub struct Gate(pub Mutex<()>);
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn commit(c: &mut rusqlite::Connection, rows: &[RawLive], snapshot: &Value) -> Result<(), String> {
    let tx = c
        .transaction()
        .map_err(|_| "Could not start live-data transaction")?;
    let old: Option<String> = tx
        .query_row("SELECT json FROM meta WHERE key='live.dirty'", [], |r| {
            r.get(0)
        })
        .ok();
    let mut dirty: BTreeSet<String> = old
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let zone = time::parse_tz(
        snapshot
            .get("timezone")
            .and_then(Value::as_str)
            .unwrap_or("UTC"),
    );
    for raw in rows {
        for m in live_ingest::normalize(raw)? {
            dirty.insert(format!(
                "{}|{}",
                m.metric.key(),
                time::day_key(aggregate::day_of(&m, &zone))
            ));
            tx.execute(
                "INSERT OR IGNORE INTO measurements(id,metric,start,json) VALUES(?1,?2,?3,?4)",
                params![
                    m.id,
                    m.metric.key(),
                    m.start,
                    serde_json::to_string(&m).map_err(|_| "Invalid measurement")?
                ],
            )
            .map_err(|_| "Could not save live measurement")?;
        }
    }
    for (key, value) in [
        ("live.dirty", serde_json::to_string(&dirty).unwrap()),
        ("live.snapshot", snapshot.to_string()),
        (
            "live.progress",
            json!({"seq":rows.last().map(|r|r.seq),"session":rows.last().map(|r|&r.session)})
                .to_string(),
        ),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO meta(key,json) VALUES(?1,?2)",
            params![key, value],
        )
        .map_err(|_| "Could not save live recovery state")?;
    }
    tx.commit()
        .map_err(|_| "Could not commit live measurements".into())
}
fn summaries(c: &mut rusqlite::Connection, timezone: &str) -> Result<(), String> {
    let Some(dirty) = db::get_keyed(c, "meta", "live.dirty")? else {
        return Ok(());
    };
    let keys: BTreeSet<String> =
        serde_json::from_str(&dirty).map_err(|_| "Invalid summary recovery state")?;
    if keys.is_empty() {
        return Ok(());
    }
    let ctx = SummaryContext {
        tz: time::parse_tz(timezone),
        now: now(),
        demo: false,
    };
    let mut changes = Vec::new();
    for key in &keys {
        let Some((metric, day)) = key.split_once('|') else {
            continue;
        };
        let (Some(metric), Some(day)) = (Metric::from_key(metric), time::parse_day(day)) else {
            continue;
        };
        let start = time::day_start(day, &ctx.tz);
        let raw = db::measurements(
            c,
            Some(metric.key()),
            start - 2 * time::DAY_MS,
            start + 3 * time::DAY_MS,
        )?;
        let ms: Vec<Measurement> = raw
            .iter()
            .map(|s| serde_json::from_str(s))
            .collect::<Result<_, _>>()
            .map_err(|_| "Invalid stored measurement")?;
        let summary = aggregate::summarize(&ms, &ctx)
            .into_iter()
            .find(|s| s.day == time::day_key(day));
        if let Some(s) = summary {
            changes.push((
                key.clone(),
                serde_json::to_string(&s).map_err(|_| "Invalid daily summary")?,
            ));
        }
    }
    let tx = c
        .transaction()
        .map_err(|_| "Could not start summary transaction")?;
    for (key, value) in changes {
        tx.execute(
            "INSERT OR REPLACE INTO summaries(key,json) VALUES(?1,?2)",
            params![key, value],
        )
        .map_err(|_| "Could not save daily summary")?;
    }
    tx.execute("DELETE FROM meta WHERE key='live.dirty'", [])
        .map_err(|_| "Could not clear summary recovery marker")?;
    tx.commit()
        .map_err(|_| "Could not commit daily summaries".into())
}
fn reconcile_locked(app: &AppHandle) -> Result<Value, String> {
    let snapshot = app.air().call("status", json!({}))?;
    let pending = app.air().call("drain", json!({}))?;
    let rows: Vec<RawLive> =
        serde_json::from_value(pending.get("rows").cloned().unwrap_or(json!([])))
            .map_err(|_| "Invalid native recording envelope")?;
    if !rows.is_empty() {
        db::with(app, "personal", |c| commit(c, &rows, &snapshot))?;
        app.air()
            .call("ack", json!({"seq":rows.last().unwrap().seq}))?;
    }
    db::with(app, "personal", |c| {
        summaries(
            c,
            snapshot
                .get("timezone")
                .and_then(Value::as_str)
                .unwrap_or("UTC"),
        )
    })?;
    Ok(snapshot)
}
pub fn launch(app: &AppHandle) {
    app.manage(Gate(Mutex::new(())));
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3));
                let gate = app.state::<Gate>();
                if let Ok(_guard) = gate.0.lock() {
                    if reconcile_locked(&app).is_err() {
                        let _ = app.air().call("stop", json!({}));
                    }
                }
            }
        });
    }
}
#[tauri::command]
pub async fn live_action(app: AppHandle, action: String) -> Result<Value, String> {
    if !matches!(
        action.as_str(),
        "status" | "start" | "stop" | "importCredentials" | "forget"
    ) {
        return Err("Unknown live action".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let gate = app.state::<Gate>();
        let _guard = gate.0.lock().map_err(|_| "Live ingestion is unavailable")?;
        if action != "status" {
            app.air().call(&action, json!({}))?;
        }
        reconcile_locked(&app)
    })
    .await
    .map_err(|_| "Live operation interrupted".to_string())?
}
pub fn clear(app: &AppHandle) -> Result<(), String> {
    let gate = app.state::<Gate>();
    let _guard = gate.0.lock().map_err(|_| "Live ingestion is unavailable")?;
    #[cfg(target_os = "android")]
    {
        app.air().call("stop", json!({}))?;
        app.air().call("clearPending", json!({}))?;
    }
    db::with(app, "personal", db::clear)
}
