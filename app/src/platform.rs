//! Platform bridge. The same WebAssembly UI runs in a browser (PWA) and in
//! the Tauri Android shell. Native capabilities are reached through Tauri
//! commands; everything here degrades gracefully in a plain browser.

use js_sys::{Function, Promise, Reflect};
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

pub fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

pub fn now_ms() -> i64 {
    js_sys::Date::now() as i64
}

/// True inside the Tauri shell (Android app or desktop dev build).
pub fn is_tauri() -> bool {
    Reflect::get(&window(), &"__TAURI_INTERNALS__".into()).map(|v| !v.is_undefined()).unwrap_or(false)
}

/// True on Android (inside the Tauri shell).
pub fn is_android() -> bool {
    is_tauri() && window().navigator().user_agent().map(|ua| ua.contains("Android")).unwrap_or(false)
}

fn to_js<T: Serialize>(v: &T) -> Result<JsValue, String> {
    v.serialize(&serde_wasm_bindgen::Serializer::json_compatible()).map_err(|e| e.to_string())
}

/// Calls a Tauri command. Fails cleanly outside Tauri.
pub async fn invoke<T: DeserializeOwned>(cmd: &str, args: impl Serialize) -> Result<T, String> {
    let internals = Reflect::get(&window(), &"__TAURI_INTERNALS__".into()).map_err(|_| "not in Tauri".to_string())?;
    if internals.is_undefined() {
        return Err("Native features are only available in the Android app.".into());
    }
    let f: Function = Reflect::get(&internals, &"invoke".into()).map_err(|_| "invoke missing")?.dyn_into().map_err(|_| "invoke not a function")?;
    let p: Promise = f.call2(&internals, &cmd.into(), &to_js(&args)?).map_err(js_err)?.dyn_into().map_err(|_| "invoke did not return a promise")?;
    let v = JsFuture::from(p).await.map_err(js_err)?;
    if v.is_undefined() || v.is_null() {
        return serde_json::from_str("null").map_err(|e| e.to_string());
    }
    serde_wasm_bindgen::from_value(v).map_err(|e| e.to_string())
}

pub fn js_err(v: JsValue) -> String {
    if let Some(s) = v.as_string() {
        return s;
    }
    if let Some(e) = v.dyn_ref::<js_sys::Error>() {
        return e.message().into();
    }
    js_sys::JSON::stringify(&v).ok().and_then(|s| s.as_string()).unwrap_or_else(|| "Unknown error".into())
}

// ---- Local storage (UI state and non-secret settings only) --------------

pub fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

pub fn ls_get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

pub fn ls_set(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

pub fn ls_remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

pub fn ls_get_json<T: DeserializeOwned>(key: &str) -> Option<T> {
    serde_json::from_str(&ls_get(key)?).ok()
}

pub fn ls_set_json<T: Serialize>(key: &str, v: &T) {
    if let Ok(s) = serde_json::to_string(v) {
        ls_set(key, &s);
    }
}

// ---- Environment ---------------------------------------------------------

pub fn detect_timezone() -> String {
    let fmt = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &js_sys::Object::new());
    Reflect::get(&fmt.resolved_options(), &"timeZone".into()).ok().and_then(|v| v.as_string()).unwrap_or_else(|| "UTC".into())
}

pub fn media_matches(q: &str) -> bool {
    window().match_media(q).ok().flatten().map(|m| m.matches()).unwrap_or(false)
}

pub fn set_root_attr(name: &str, value: &str) {
    if let Some(el) = window().document().and_then(|d| d.document_element()) {
        let _ = el.set_attribute(name, value);
    }
}

pub fn set_theme_color(color: &str) {
    if let Some(doc) = window().document() {
        if let Ok(Some(m)) = doc.query_selector("meta[name=theme-color]") {
            let _ = m.set_attribute("content", color);
        }
    }
}

pub fn is_online() -> bool {
    window().navigator().on_line()
}

/// Light haptic tick on Android for meaningful actions only. No-op elsewhere.
pub fn haptic(enabled: bool) {
    if !enabled || !is_android() {
        return;
    }
    leptos::task::spawn_local(async {
        let _ = invoke::<()>("plugin:haptics|impact_feedback", serde_json::json!({"style": "light"})).await;
    });
}

/// Saves a file: a download in the browser, a system "save as" dialog on Android.
pub async fn save_file(name: &str, mime: &str, content: &str) -> Result<String, String> {
    if is_tauri() {
        let r: Option<String> = invoke("export_file", serde_json::json!({"name": name, "content": content})).await?;
        return r.ok_or_else(|| "Export cancelled.".into());
    }
    let parts = js_sys::Array::of1(&JsValue::from_str(content));
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type(mime);
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag).map_err(js_err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_err)?;
    let doc = window().document().ok_or("no document")?;
    let a: web_sys::HtmlAnchorElement = doc.create_element("a").map_err(js_err)?.dyn_into().map_err(|_| "anchor")?;
    a.set_href(&url);
    a.set_download(name);
    a.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(name.to_string())
}

/// Reads a user-selected file as text.
pub async fn read_file(file: &web_sys::File) -> Result<String, String> {
    let p = file.text();
    JsFuture::from(p).await.map_err(js_err)?.as_string().ok_or_else(|| "The file is not text.".into())
}

pub fn focus_by_id(id: &str) {
    if let Some(el) = window().document().and_then(|d| d.get_element_by_id(id)) {
        if let Ok(h) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = h.focus();
        }
    }
}

pub fn focus_selector(sel: &str) {
    if let Some(el) = window().document().and_then(|d| d.query_selector(sel).ok().flatten()) {
        if let Ok(h) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = h.focus();
        }
    }
}

pub fn scroll_into_view(sel: &str) {
    if let Some(el) = window().document().and_then(|d| d.query_selector(sel).ok().flatten()) {
        let opts = web_sys::ScrollIntoViewOptions::new();
        opts.set_block(web_sys::ScrollLogicalPosition::Center);
        el.scroll_into_view_with_scroll_into_view_options(&opts);
    }
}

pub fn scroll_y() -> f64 {
    window().scroll_y().unwrap_or(0.0)
}

pub struct ScrollMemory(pub f64);

thread_local! {
    static SCROLL_MEMORY: std::cell::RefCell<ScrollMemory> = const { std::cell::RefCell::new(ScrollMemory(0.0)) };
}

pub fn with_scroll_memory<T>(f: impl FnOnce(&mut ScrollMemory) -> T) -> T {
    SCROLL_MEMORY.with(|m| f(&mut m.borrow_mut()))
}

pub fn scroll_top() {
    window().scroll_to_with_x_and_y(0.0, 0.0);
}

/// Registers the service worker (PWA only; never inside Tauri).
pub fn register_service_worker() {
    if is_tauri() {
        return;
    }
    let nav = window().navigator();
    if Reflect::has(&nav, &"serviceWorker".into()).unwrap_or(false) {
        let _ = nav.service_worker().register("./sw.js");
    }
}

/// `fetch` for the browser AI adapter. Returns (status, body).
pub async fn http(method: &str, url: &str, headers: &[(String, String)], body: Option<&str>) -> Result<(u16, String), String> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_mode(web_sys::RequestMode::Cors);
    let h = web_sys::Headers::new().map_err(js_err)?;
    for (k, v) in headers {
        h.set(k, v).map_err(js_err)?;
    }
    init.set_headers(&h);
    if let Some(b) = body {
        init.set_body(&JsValue::from_str(b));
    }
    let req = web_sys::Request::new_with_str_and_init(url, &init).map_err(js_err)?;
    let resp = JsFuture::from(window().fetch_with_request(&req)).await.map_err(|_| {
        "The browser blocked the request or could not reach the provider. Many AI providers do not allow direct requests from web pages (CORS). Use the Android app, or a provider that supports browser access.".to_string()
    })?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "bad response")?;
    let text = JsFuture::from(resp.text().map_err(js_err)?).await.map_err(js_err)?.as_string().unwrap_or_default();
    Ok((resp.status(), text))
}
