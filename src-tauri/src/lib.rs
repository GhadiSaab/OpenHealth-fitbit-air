//! Airspace native shell.
//!
//! The WebAssembly UI calls these commands for everything that needs the
//! platform: SQLite storage, secret storage, native HTTPS for the optional
//! AI coach (no CORS, key never exposed to the webview), file export and
//! reminders. No command logs health data or secrets.

mod db;

use airspace_core::ai::{self, AiProviderKind, HttpSpec};
use std::time::Duration;
use tauri::Manager;
use tauri_plugin_vault::VaultExt;

const AI_KEY: &str = "ai_api_key";
/// The endpoint the key was saved for; the key is only ever sent there.
const AI_ENDPOINT: &str = "ai_api_endpoint";

type CmdResult<T> = Result<T, String>;

// ---- Storage -------------------------------------------------------------

#[tauri::command]
async fn db_open(app: tauri::AppHandle, db: String) -> CmdResult<()> {
    db::with(&app, &db, |_| Ok(()))
}

#[tauri::command]
async fn db_put_measurements(app: tauri::AppHandle, db: String, rows: Vec<String>) -> CmdResult<()> {
    db::with(&app, &db, |c| db::put_measurements(c, &rows))
}

#[tauri::command]
async fn db_measurements(app: tauri::AppHandle, db: String, metric: Option<String>, from: i64, to: i64) -> CmdResult<Vec<String>> {
    db::with(&app, &db, |c| db::measurements(c, metric.as_deref(), from, to))
}

#[tauri::command]
async fn db_all(app: tauri::AppHandle, db: String, table: String) -> CmdResult<Vec<String>> {
    db::with(&app, &db, |c| db::all(c, &table))
}

#[tauri::command]
async fn db_put_keyed(app: tauri::AppHandle, db: String, table: String, rows: Vec<(String, String)>) -> CmdResult<()> {
    db::with(&app, &db, |c| db::put_keyed(c, &table, &rows))
}

#[tauri::command]
async fn db_delete_keyed(app: tauri::AppHandle, db: String, table: String, keys: Vec<String>) -> CmdResult<()> {
    db::with(&app, &db, |c| db::delete_keyed(c, &table, &keys))
}

#[tauri::command]
async fn db_meta_get(app: tauri::AppHandle, db: String, key: String) -> CmdResult<Option<String>> {
    db::with(&app, &db, |c| db::get_keyed(c, "meta", &key))
}

#[tauri::command]
async fn db_clear(app: tauri::AppHandle, db: String) -> CmdResult<()> {
    db::with(&app, &db, db::clear)
}

#[tauri::command]
async fn db_count(app: tauri::AppHandle, db: String) -> CmdResult<u32> {
    db::with(&app, &db, db::count)
}

// ---- Files ----------------------------------------------------------------

/// Shows the system "save as" dialog and writes `content`. Returns the file
/// name, or `None` if the user cancelled.
#[tauri::command]
async fn export_file(app: tauri::AppHandle, name: String, content: String) -> CmdResult<Option<String>> {
    use std::io::Write;
    use tauri_plugin_dialog::DialogExt;
    use tauri_plugin_fs::{FsExt, OpenOptions};
    let ext = name.rsplit('.').next().unwrap_or("txt").to_string();
    let Some(path) = app.dialog().file().set_file_name(&name).add_filter(ext.to_uppercase(), &[ext.as_str()]).blocking_save_file() else {
        return Ok(None);
    };
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    let mut f = app.fs().open(path, opts).map_err(|e| format!("Couldn't create the file: {e}"))?;
    f.write_all(content.as_bytes()).map_err(|e| format!("Couldn't write the file: {e}"))?;
    Ok(Some(name))
}

// ---- Optional AI coach -----------------------------------------------------

/// Stores the key together with the endpoint it may be sent to.
#[tauri::command]
async fn ai_key_set(app: tauri::AppHandle, key: String, endpoint: String) -> CmdResult<()> {
    let key = key.trim();
    if key.is_empty() || key.len() > 512 {
        return Err("That doesn't look like an API key.".into());
    }
    if !allowed_url(&endpoint) {
        return Err("The endpoint must use HTTPS.".into());
    }
    app.vault().set(AI_KEY, key).map_err(|e| e.to_string())?;
    app.vault().set(AI_ENDPOINT, endpoint.trim()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ai_key_delete(app: tauri::AppHandle) -> CmdResult<()> {
    app.vault().delete(AI_KEY).map_err(|e| e.to_string())?;
    app.vault().delete(AI_ENDPOINT).map_err(|e| e.to_string())
}

/// The endpoint the stored key is bound to, if any.
#[tauri::command]
async fn ai_key_endpoint(app: tauri::AppHandle) -> CmdResult<Option<String>> {
    if app.vault().get(AI_KEY).map_err(|e| e.to_string())?.is_none() {
        return Ok(None);
    }
    app.vault().get(AI_ENDPOINT).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ai_key_status(app: tauri::AppHandle) -> CmdResult<bool> {
    Ok(app.vault().get(AI_KEY).map_err(|e| e.to_string())?.is_some())
}

fn allowed_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    (u.starts_with("https://") || u.starts_with("http://localhost") || u.starts_with("http://127.0.0.1")) && !u.contains('?') && !u.contains('#')
}

/// Sends a request built by `airspace_core::ai`, adding the key from the
/// vault. Returns (status, body). The key never reaches the webview.
#[tauri::command]
async fn ai_request(app: tauri::AppHandle, provider: AiProviderKind, spec: HttpSpec) -> CmdResult<(u16, String)> {
    if !allowed_url(&spec.url) {
        return Err("The endpoint must use HTTPS.".into());
    }
    let key = app.vault().get(AI_KEY).map_err(|e| e.to_string())?.ok_or("No API key is stored. Add one in Coach settings.")?;
    // The key only goes to the endpoint it was saved for, whatever the webview asks.
    let bound = app.vault().get(AI_ENDPOINT).map_err(|e| e.to_string())?.unwrap_or_default();
    if !ai::url_matches_endpoint(&spec.url, &bound) {
        return Err("This request isn't for the endpoint your key was saved with. Re-enter the key for the new endpoint.".into());
    }
    let (hk, hv) = ai::provider(provider).auth_header(&key);
    tauri::async_runtime::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(180)))
            .build()
            .into();
        let result = if spec.method == "GET" {
            let mut r = agent.get(&spec.url);
            for (k, v) in &spec.headers {
                r = r.header(k.as_str(), v.as_str());
            }
            r.header(hk.as_str(), hv.as_str()).call()
        } else {
            let mut r = agent.post(&spec.url);
            for (k, v) in &spec.headers {
                r = r.header(k.as_str(), v.as_str());
            }
            r.header(hk.as_str(), hv.as_str()).send(spec.body.unwrap_or_default().as_str())
        };
        match result {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let body = resp.into_body().read_to_string().unwrap_or_default();
                Ok((status, body))
            }
            // Deliberately generic: transport errors can echo request details.
            Err(_) => Err("Couldn't reach the provider. Check your connection and the endpoint.".to_string()),
        }
    })
    .await
    .map_err(|_| "Request failed".to_string())?
}

// ---- Reminders ---------------------------------------------------------------

const REMINDER_ID: i32 = 4101;

#[tauri::command]
async fn reminders_set(app: tauri::AppHandle, enabled: bool, time: String) -> CmdResult<()> {
    #[cfg(mobile)]
    {
        use tauri_plugin_notification::{NotificationExt, PermissionState, Schedule, ScheduleInterval};
        let n = app.notification();
        let _ = n.cancel(vec![REMINDER_ID]);
        if !enabled {
            return Ok(());
        }
        if !matches!(n.permission_state().map_err(|e| e.to_string())?, PermissionState::Granted)
            && !matches!(n.request_permission().map_err(|e| e.to_string())?, PermissionState::Granted)
        {
            return Err("Notifications are turned off for Airspace in system settings.".into());
        }
        let (h, m) = time.split_once(':').and_then(|(h, m)| Some((h.parse::<u8>().ok()?, m.parse::<u8>().ok()?))).ok_or("Invalid time")?;
        n.builder()
            .id(REMINDER_ID)
            .title("Airspace")
            .body("A moment to add today's journal note, if you'd like.")
            .schedule(Schedule::Interval { interval: ScheduleInterval { hour: Some(h), minute: Some(m), ..Default::default() }, allow_while_idle: false })
            .show()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(desktop)]
    {
        let _ = (app, enabled, time, REMINDER_ID);
        Err("Reminders are available in the Android app.".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_vault::init())
        .plugin(tauri_plugin_haptics::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            app.manage(db::Dbs::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            db_open,
            db_put_measurements,
            db_measurements,
            db_all,
            db_put_keyed,
            db_delete_keyed,
            db_meta_get,
            db_clear,
            db_count,
            export_file,
            ai_key_set,
            ai_key_delete,
            ai_key_status,
            ai_key_endpoint,
            ai_request,
            reminders_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Airspace");
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_insecure_urls() {
        assert!(super::allowed_url("https://api.anthropic.com/v1/messages"));
        assert!(!super::allowed_url("http://evil.example/v1"));
        assert!(!super::allowed_url("https://x.example/v1?key=1"));
    }
}
