//! IndexedDB repository for the PWA.
//!
//! Database names: `airspace-personal`, `airspace-demo` (never shared).
//! Each stored value is `{ key fields…, json }`, where `json` is the
//! serialized core type; key fields exist only for indexing.

use crate::platform::{js_err, window};
use airspace_core::DB_SCHEMA_VERSION;
use js_sys::{Array, Object, Promise, Reflect};
use std::cell::RefCell;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbKeyRange, IdbObjectStore, IdbRequest, IdbTransaction, IdbTransactionMode};

pub const MEASUREMENTS: &str = "measurements";
pub const SUMMARIES: &str = "summaries";
pub const JOURNAL: &str = "journal";
pub const BATCHES: &str = "batches";
pub const META: &str = "meta";

thread_local! {
    static DBS: RefCell<Vec<(String, IdbDatabase)>> = const { RefCell::new(vec![]) };
}

fn request(r: &IdbRequest) -> Promise {
    let r = r.clone();
    Promise::new(&mut |resolve, reject| {
        let r2 = r.clone();
        let ok = Closure::once_into_js(move |_e: web_sys::Event| {
            let _ = resolve.call1(&JsValue::NULL, &r2.result().unwrap_or(JsValue::UNDEFINED));
        });
        let r3 = r.clone();
        let err = Closure::once_into_js(move |_e: web_sys::Event| {
            let msg = r3.error().ok().flatten().map(|e| e.message()).unwrap_or_else(|| "IndexedDB request failed".into());
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str(&msg));
        });
        r.set_onsuccess(Some(ok.unchecked_ref()));
        r.set_onerror(Some(err.unchecked_ref()));
    })
}

async fn await_req(r: Result<IdbRequest, JsValue>) -> Result<JsValue, String> {
    let r = r.map_err(js_err)?;
    JsFuture::from(request(&r)).await.map_err(js_err)
}

fn tx_done(tx: &IdbTransaction) -> Promise {
    let tx = tx.clone();
    Promise::new(&mut |resolve, reject| {
        let ok = Closure::once_into_js(move |_e: web_sys::Event| {
            let _ = resolve.call0(&JsValue::NULL);
        });
        let rj = reject.clone();
        let err = Closure::once_into_js(move |_e: web_sys::Event| {
            let _ = rj.call1(&JsValue::NULL, &JsValue::from_str("IndexedDB transaction failed (storage may be full)"));
        });
        let abort = Closure::once_into_js(move |_e: web_sys::Event| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("IndexedDB transaction was aborted"));
        });
        tx.set_oncomplete(Some(ok.unchecked_ref()));
        tx.set_onerror(Some(err.unchecked_ref()));
        tx.set_onabort(Some(abort.unchecked_ref()));
    })
}

/// Versioned migrations. Each step runs once when upgrading past it.
fn migrate(db: &IdbDatabase, old: u32) -> Result<(), JsValue> {
    if old < 1 {
        let p = web_sys::IdbObjectStoreParameters::new();
        p.set_key_path(&JsValue::from_str("id"));
        let m = db.create_object_store_with_optional_parameters(MEASUREMENTS, &p)?;
        let idx = Array::of2(&"metric".into(), &"start".into());
        m.create_index_with_str_sequence("metric_start", &idx)?;
        m.create_index_with_str("start", "start")?;
        let p = web_sys::IdbObjectStoreParameters::new();
        p.set_key_path(&JsValue::from_str("key"));
        db.create_object_store_with_optional_parameters(SUMMARIES, &p)?;
        let p = web_sys::IdbObjectStoreParameters::new();
        p.set_key_path(&JsValue::from_str("id"));
        db.create_object_store_with_optional_parameters(JOURNAL, &p)?;
        let p = web_sys::IdbObjectStoreParameters::new();
        p.set_key_path(&JsValue::from_str("id"));
        db.create_object_store_with_optional_parameters(BATCHES, &p)?;
        let p = web_sys::IdbObjectStoreParameters::new();
        p.set_key_path(&JsValue::from_str("key"));
        db.create_object_store_with_optional_parameters(META, &p)?;
    }
    // Future: if old < 2 { … }
    Ok(())
}

pub async fn open(name: &str) -> Result<IdbDatabase, String> {
    if let Some(db) = DBS.with(|d| d.borrow().iter().find(|(n, _)| n == name).map(|(_, db)| db.clone())) {
        return Ok(db);
    }
    let factory = window().indexed_db().map_err(js_err)?.ok_or("IndexedDB is not available in this browser (private mode?)")?;
    let req = factory.open_with_u32(name, DB_SCHEMA_VERSION).map_err(js_err)?;
    let r2 = req.clone();
    let upgrade = Closure::<dyn FnMut(web_sys::IdbVersionChangeEvent)>::new(move |e: web_sys::IdbVersionChangeEvent| {
        if let Ok(db) = r2.result().and_then(|v| v.dyn_into::<IdbDatabase>()) {
            let _ = migrate(&db, e.old_version() as u32);
        }
    });
    req.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
    let v = JsFuture::from(request(&req)).await.map_err(js_err)?;
    drop(upgrade);
    let db: IdbDatabase = v.dyn_into().map_err(|_| "could not open database")?;
    DBS.with(|d| d.borrow_mut().push((name.to_string(), db.clone())));
    Ok(db)
}

fn store(db: &IdbDatabase, name: &str, mode: IdbTransactionMode) -> Result<(IdbTransaction, IdbObjectStore), String> {
    let tx = db.transaction_with_str_and_mode(name, mode).map_err(js_err)?;
    let st = tx.object_store(name).map_err(js_err)?;
    Ok((tx, st))
}

fn obj(fields: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (k, v) in fields {
        let _ = Reflect::set(&o, &JsValue::from_str(k), v);
    }
    o.into()
}

/// Writes rows in one transaction.
pub async fn put_rows(db: &IdbDatabase, name: &str, rows: Vec<JsValue>) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    let (tx, st) = store(db, name, IdbTransactionMode::Readwrite)?;
    let done = tx_done(&tx);
    for r in rows {
        st.put(&r).map_err(js_err)?;
    }
    JsFuture::from(done).await.map_err(js_err)?;
    Ok(())
}

pub async fn delete_keys(db: &IdbDatabase, name: &str, keys: &[String]) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    let (tx, st) = store(db, name, IdbTransactionMode::Readwrite)?;
    let done = tx_done(&tx);
    for k in keys {
        st.delete(&JsValue::from_str(k)).map_err(js_err)?;
    }
    JsFuture::from(done).await.map_err(js_err)?;
    Ok(())
}

pub fn measurement_row(id: &str, metric: &str, start: i64, json: String) -> JsValue {
    obj(&[("id", id.into()), ("metric", metric.into()), ("start", JsValue::from_f64(start as f64)), ("json", json.into())])
}

pub fn keyed_row(key_field: &str, key: &str, json: String) -> JsValue {
    obj(&[(key_field, key.into()), ("json", json.into())])
}

fn json_of(v: &JsValue) -> Option<String> {
    Reflect::get(v, &"json".into()).ok()?.as_string()
}

fn rows_to_json(v: JsValue) -> Vec<String> {
    v.dyn_into::<Array>().map(|a| a.iter().filter_map(|r| json_of(&r)).collect()).unwrap_or_default()
}

pub async fn all_json(db: &IdbDatabase, name: &str) -> Result<Vec<String>, String> {
    let (_tx, st) = store(db, name, IdbTransactionMode::Readonly)?;
    Ok(rows_to_json(await_req(st.get_all()).await?))
}

/// Measurements with `from <= start < to`, optionally for one metric.
pub async fn measurements_json(db: &IdbDatabase, metric: Option<&str>, from: i64, to: i64) -> Result<Vec<String>, String> {
    let (_tx, st) = store(db, MEASUREMENTS, IdbTransactionMode::Readonly)?;
    let res = match metric {
        Some(m) => {
            let lo = Array::of2(&m.into(), &JsValue::from_f64(from as f64));
            let hi = Array::of2(&m.into(), &JsValue::from_f64(to as f64));
            let range = IdbKeyRange::bound_with_lower_open_and_upper_open(&lo, &hi, false, true).map_err(js_err)?;
            let idx = st.index("metric_start").map_err(js_err)?;
            await_req(idx.get_all_with_key(&range)).await?
        }
        None => {
            let range = IdbKeyRange::bound_with_lower_open_and_upper_open(&JsValue::from_f64(from as f64), &JsValue::from_f64(to as f64), false, true)
                .map_err(js_err)?;
            let idx = st.index("start").map_err(js_err)?;
            await_req(idx.get_all_with_key(&range)).await?
        }
    };
    Ok(rows_to_json(res))
}

pub async fn get_json(db: &IdbDatabase, name: &str, key: &str) -> Result<Option<String>, String> {
    let (_tx, st) = store(db, name, IdbTransactionMode::Readonly)?;
    let v = await_req(st.get(&JsValue::from_str(key))).await?;
    if v.is_undefined() { Ok(None) } else { Ok(json_of(&v)) }
}

pub async fn count(db: &IdbDatabase, name: &str) -> Result<u32, String> {
    let (_tx, st) = store(db, name, IdbTransactionMode::Readonly)?;
    Ok(await_req(st.count()).await?.as_f64().unwrap_or(0.0) as u32)
}

pub async fn clear_all(db: &IdbDatabase) -> Result<(), String> {
    for name in [MEASUREMENTS, SUMMARIES, JOURNAL, BATCHES, META] {
        let (tx, st) = store(db, name, IdbTransactionMode::Readwrite)?;
        let done = tx_done(&tx);
        st.clear().map_err(js_err)?;
        JsFuture::from(done).await.map_err(js_err)?;
    }
    Ok(())
}
