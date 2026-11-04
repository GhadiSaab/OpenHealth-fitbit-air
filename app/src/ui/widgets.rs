//! Reusable interface components.

use crate::state::{App, Route, Sheet};
use crate::ui::icons::Icon;
use airspace_core::aggregate::SummaryExtra;
use airspace_core::catalog;
use airspace_core::time;
use airspace_core::{DbKind, Origin};
use leptos::prelude::*;

pub fn use_app() -> App {
    expect_context::<App>()
}

/// Screen header: optional back button, title, and the profile/device entry.
#[component]
pub fn Header(#[prop(into)] title: String, #[prop(optional)] back: bool, #[prop(optional)] subtitle: Option<String>) -> impl IntoView {
    let app = use_app();
    let status = move || {
        if app.db.get() == DbKind::Demo {
            ("demo", "Demo data. Device and data settings")
        } else {
            ("off", "Device not connected. Device and data settings")
        }
    };
    view! {
        <header class="screen-header">
            {back.then(|| view! {
                <button class="icon-btn" on:click=move |_| app.back() aria-label="Back"><Icon name="back" /></button>
            })}
            <div class="header-titles">
                <h1 id="screen-title" tabindex="-1">{title}</h1>
                {subtitle.map(|s| view! { <p class="header-sub">{s}</p> })}
            </div>
            <button class="profile-btn" on:click=move |_| app.go(Route::Profile) aria-label=move || status().1>
                <Icon name="device" />
                <span class=move || format!("status-dot {}", status().0) aria-hidden="true"></span>
            </button>
        </header>
    }
}

#[component]
pub fn TabBar() -> impl IntoView {
    let app = use_app();
    let tabs: [(&'static str, &'static str, Route); 4] =
        [("Today", "today", Route::Today), ("Explore", "explore", Route::Explore), ("Journal", "journal", Route::Journal), ("Coach", "coach", Route::Coach)];
    view! {
        <nav class="tabbar" aria-label="Primary">
            <div class="rail-brand" aria-hidden="true"><span class="brand-dot"></span>"Airspace"</div>
            {tabs.into_iter().enumerate().map(|(i, (label, icon, route))| {
                let active = move || app.route.get().tab() == Some(i as u8);
                view! {
                    <button
                        class="tab"
                        class:active=active
                        aria-current=move || if active() { Some("page") } else { None }
                        on:click=move |_| { app.go(route); }
                    >
                        <Icon name=icon />
                        <span>{label}</span>
                    </button>
                }
            }).collect_view()}
        </nav>
    }
}

/// Segmented control with radio semantics.
#[component]
pub fn Segmented<T>(options: Vec<(T, &'static str)>, value: Signal<T>, on_change: Callback<T>, #[prop(into)] label: String) -> impl IntoView
where
    T: Copy + PartialEq + Send + Sync + 'static,
{
    let n = options.len();
    let opts = options.clone();
    view! {
        <div class="segmented" role="radiogroup" aria-label=label>
            {options.into_iter().enumerate().map(|(i, (v, l))| {
                let opts = opts.clone();
                let checked = move || value.get() == v;
                view! {
                    <button
                        role="radio"
                        class="seg"
                        class:on=checked
                        aria-checked=move || if checked() { "true" } else { "false" }
                        tabindex=move || if checked() { "0" } else { "-1" }
                        on:click=move |_| on_change.run(v)
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                            let j = match ev.key().as_str() { "ArrowRight" | "ArrowDown" => (i + 1) % n, "ArrowLeft" | "ArrowUp" => (i + n - 1) % n, _ => return };
                            ev.prevent_default();
                            on_change.run(opts[j].0);
                        }
                    >{l}</button>
                }
            }).collect_view()}
        </div>
    }
}

/// On/off switch.
#[component]
pub fn Switch(#[prop(into)] label: String, checked: Signal<bool>, on_change: Callback<bool>, #[prop(optional)] disabled: Option<Signal<bool>>, #[prop(optional, into)] hint: Option<String>) -> impl IntoView {
    let dis = move || disabled.map(|d| d.get()).unwrap_or(false);
    let hint_id = format!("hint-{}", label.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "-"));
    let has_hint = hint.is_some();
    view! {
        // The whole row is the touch target; the switch is the single focusable control.
        <div class="row switch-row" on:click=move |ev| {
            let on_switch = ev.target().and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok()).map(|e| e.closest(".switch").ok().flatten().is_some()).unwrap_or(false);
            if !on_switch && !dis() { on_change.run(!checked.get_untracked()) }
        }>
            <div class="row-text">
                <span class="row-title">{label.clone()}</span>
                {hint.map(|h| view! { <span class="row-hint" id=hint_id.clone()>{h}</span> })}
            </div>
            <button
                role="switch"
                class="switch"
                aria-describedby=has_hint.then(|| hint_id.clone())
                aria-checked=move || if checked.get() { "true" } else { "false" }
                aria-label=label
                disabled=dis
                on:click=move |_| on_change.run(!checked.get_untracked())
            ><span class="track" aria-hidden="true"><span class="knob"></span></span></button>
        </div>
    }
}

#[component]
pub fn EmptyState(icon: &'static str, #[prop(into)] title: String, #[prop(into)] body: String, #[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <div class="empty">
            <div class="empty-icon"><Icon name=icon size=28 /></div>
            <h3>{title}</h3>
            <p>{body}</p>
            {children.map(|c| c())}
        </div>
    }
}

/// Bottom sheet / dialog. Closes on Escape and backdrop click.
#[component]
pub fn SheetFrame(#[prop(into)] title: String, on_close: Callback<()>, children: Children) -> impl IntoView {
    let id = "sheet-title";
    Effect::new(move |_| crate::platform::focus_by_id("sheet-close"));
    view! {
        <div class="sheet-backdrop" on:click=move |_| on_close.run(())></div>
        <div class="sheet" role="dialog" aria-modal="true" aria-labelledby=id
            on:keydown=move |ev: leptos::ev::KeyboardEvent| { if ev.key() == "Escape" { on_close.run(()) } }>
            <div class="sheet-grabber" aria-hidden="true"></div>
            <div class="sheet-head">
                <h2 id=id>{title}</h2>
                <button id="sheet-close" class="icon-btn" aria-label="Close" on:click=move |_| on_close.run(())><Icon name="close" /></button>
            </div>
            <div class="sheet-body">{children()}</div>
        </div>
    }
}

fn origin_label(o: Origin) -> &'static str {
    match o {
        Origin::HistoryImport => "Imported history",
        Origin::BluetoothLive => "Bluetooth recording",
        Origin::Derived => "Calculated by Airspace",
    }
}

thread_local! {
    static SHEET_OPENER: std::cell::RefCell<Option<web_sys::HtmlElement>> = const { std::cell::RefCell::new(None) };
    static SHEET_PUSHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn sheet_history_pushed() -> bool {
    SHEET_PUSHED.with(|p| p.get())
}

pub fn sheet_history_popped() {
    SHEET_PUSHED.with(|p| p.set(false));
}

/// Makes everything behind an open sheet inert (unfocusable and hidden from
/// screen readers) and returns focus to the opener when it closes.
fn set_background_inert(on: bool) {
    use wasm_bindgen::JsCast;
    let Some(doc) = crate::platform::window().document() else { return };
    for sel in [".app", ".skip-link"] {
        if let Ok(Some(el)) = doc.query_selector(sel) {
            if on { let _ = el.set_attribute("inert", ""); } else { let _ = el.remove_attribute("inert"); }
        }
    }
    crate::platform::set_root_attr("data-sheet", if on { "open" } else { "closed" });
    let history = crate::platform::window().history().ok();
    if on {
        let opener = doc.active_element().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
        SHEET_OPENER.with(|o| *o.borrow_mut() = opener);
        // A history entry so the Android/browser Back button closes the sheet.
        if let Some(h) = &history {
            if h.push_state(&wasm_bindgen::JsValue::from_str("sheet"), "").is_ok() {
                SHEET_PUSHED.with(|p| p.set(true));
            }
        }
    } else {
        // Closed by a button: drop the entry we pushed (Back already did otherwise).
        if SHEET_PUSHED.with(|p| p.replace(false)) {
            if let Some(h) = &history {
                let _ = h.back();
            }
        }
        if let Some(el) = SHEET_OPENER.with(|o| o.borrow_mut().take()) {
            if el.is_connected() {
                let _ = el.focus();
            }
        }
    }
}

/// Global sheet host (provenance and status explanations).
#[component]
pub fn SheetHost() -> impl IntoView {
    let app = use_app();
    let close = Callback::new(move |_| app.sheet.set(None));
    Effect::new(move |prev: Option<bool>| {
        let open = app.sheet.with(|s| s.is_some());
        if prev != Some(open) && (open || prev.is_some()) {
            set_background_inert(open);
        }
        open
    });
    move || {
        app.sheet.get().map(|s| match s {
            Sheet::Provenance(metric, day) => {
                let info = catalog::info(metric);
                let tz = app.tz_untracked();
                let s = app.summaries.with_untracked(|i| i.get(metric, day).cloned());
                let settings = app.settings.get_untracked();
                view! {
                    <SheetFrame title=format!("{} · {}", info.title, day.format("%a %-d %b %Y")) on_close=close>
                        {match s {
                            None => view! { <p>"No data was recorded for this day. Missing days are shown as gaps, never as zero."</p> }.into_any(),
                            Some(s) => view! {
                                <dl class="facts">
                                    <dt>"Value"</dt>
                                    <dd class="tabular">{s.value.map(|v| airspace_core::settings::display(metric, v, &settings).joined()).unwrap_or_else(|| "No usable value".into())}</dd>
                                    <dt>"How it was produced"</dt><dd>{s.method.clone()}</dd>
                                    <dt>"Source"</dt><dd>{s.origins.iter().map(|o| origin_label(*o)).collect::<Vec<_>>().join(" + ")}{if s.demo { " (simulated demo data)" } else { "" }}</dd>
                                    <dt>"Records"</dt><dd class="tabular">{crate::ui::format::count(s.count as usize)}</dd>
                                    {s.coverage.map(|c| view! { <dt>"Completeness"</dt><dd class="tabular">{format!("{:.0}% of the day has data", c * 100.0)}</dd> })}
                                    {s.latest_ts.map(|t| view! { <dt>"Latest measurement"</dt><dd class="tabular">{time::format_ts(t, &tz, "%a %-d %b, %H:%M")}</dd> })}
                                    {s.provisional.then(|| view! { <dt>"Field mapping"</dt><dd>"Provisional: some values come from a live Bluetooth mapping that has not been validated yet."</dd> })}
                                    {match &s.extra {
                                        Some(SummaryExtra::Additive { parts, overlapped, rejected_jumps, .. }) if parts.len() > 1 || *overlapped > 0 || *rejected_jumps > 0 => Some(view! {
                                            <dt>"Breakdown"</dt>
                                            <dd><ul class="plain">
                                                {parts.iter().map(|p| view! { <li class="tabular">{format!("{}: {} ({}–{})", p.label, airspace_core::settings::display(metric, p.value, &settings).joined(), time::format_ts(p.from, &tz, "%H:%M"), time::format_ts(p.to, &tz, "%H:%M"))}</li> }).collect_view()}
                                                {(*overlapped > 0).then(|| view! { <li>{format!("{overlapped} overlapping records were left out to avoid double counting.")}</li> })}
                                                {(*rejected_jumps > 0).then(|| view! { <li>{format!("{rejected_jumps} implausible counter jumps were ignored.")}</li> })}
                                            </ul></dd>
                                        }.into_any()),
                                        Some(SummaryExtra::Hrv { method, excluded_other_methods }) => Some(view! {
                                            <dt>"Method"</dt>
                                            <dd>{format!("{}{}", method.label(), if *excluded_other_methods > 0 { format!(". {excluded_other_methods} value(s) measured with a different method were kept separate.") } else { String::new() })}</dd>
                                        }.into_any()),
                                        Some(SummaryExtra::Zones { origin, .. }) => Some(view! {
                                            <dt>"Zone boundaries"</dt>
                                            <dd>{match origin { airspace_core::ZoneOrigin::Imported => "Imported from the source", airspace_core::ZoneOrigin::Calculated => "Calculated by Airspace" }}</dd>
                                        }.into_any()),
                                        Some(SummaryExtra::Sleep { stages_measured, asleep_derived, .. }) => Some(view! {
                                            <dt>"Measured vs calculated"</dt>
                                            <dd>{format!("Start and end times are measured. {} {}",
                                                if *stages_measured { "Stage intervals come from the source; stage totals are calculated by Airspace." } else { "This night has no stage data." },
                                                if *asleep_derived { "Time asleep was calculated from stages." } else { "Time asleep is as reported by the source." })}</dd>
                                        }.into_any()),
                                        _ => None,
                                    }}
                                    <dt>"Calculation version"</dt><dd class="tabular">{format!("v{}", s.calc_version)}</dd>
                                </dl>
                            }.into_any(),
                        }}
                        <RecordList metric=metric day=day />
                        <p class="fineprint">{info.caution}</p>
                    </SheetFrame>
                }.into_any()
            }
            Sheet::Status => view! {
                <SheetFrame title="About these readings" on_close=close>
                    <StatusExplainer />
                </SheetFrame>
            }.into_any(),
        })
    }
}

#[component]
fn StatusExplainer() -> impl IntoView {
    let app = use_app();
    let tz = app.tz_untracked();
    let now = app.now.get_untracked();
    view! {
        {move || match app.demo_device.get() {
            Some(d) if app.db.get() == DbKind::Demo => view! {
                <dl class="facts">
                    <dt>"Device"</dt><dd>{d.name.clone()}" · battery "{d.battery_pct}"% (simulated)"</dd>
                    <dt>"Connection"</dt><dd>{format!("Disconnected {} ({}).", time::fmt_ago(d.disconnected_at, now), time::format_ts(d.disconnected_at, &tz, "%H:%M"))}</dd>
                    <dt>"History"</dt><dd>{format!("Imported history covers everything up to {}.", time::format_ts(d.history_synced_through, &tz, "%a %H:%M"))}</dd>
                    <dt>"Live recording"</dt><dd>{format!("A simulated Bluetooth session ran from {} to {}. Nothing has been recorded since, so the latest values are not current.", time::format_ts(d.session_started_at, &tz, "%H:%M"), time::format_ts(d.disconnected_at, &tz, "%H:%M"))}</dd>
                </dl>
            }.into_any(),
            _ => view! {
                <dl class="facts">
                    <dt>"Connection"</dt><dd>"Not connected. Bluetooth recording is not part of this version, so nothing is recorded live."</dd>
                    <dt>"What you see"</dt><dd>"Values come from history you imported. They show when they were measured, not the current moment."</dd>
                </dl>
            }.into_any(),
        }}
        <h3 class="sub">"How to read the labels"</h3>
        <ul class="plain legend-list">
            <li><span class="pill live">"Live"</span>" received in the last 2 minutes while connected"</li>
            <li><span class="pill recent">"Recent"</span>" recorded live within the last 30 minutes"</li>
            <li><span class="pill stale">"Last reading"</span>" older than 30 minutes; not current"</li>
            <li><span class="pill hist">"History"</span>" imported from a file, not recorded live"</li>
        </ul>
    }
}

#[component]
pub fn ToastHost() -> impl IntoView {
    let app = use_app();
    Effect::new(move |_| {
        if let Some(t) = app.toast.get() {
            let id = t.id;
            set_timeout(move || if app.toast.get_untracked().map(|x| x.id) == Some(id) { app.toast.set(None) }, std::time::Duration::from_millis(if t.error { 7000 } else { 4000 }));
        }
    });
    view! {
        // Two separate live regions, so an error is announced once (assertively)
        // and an ordinary confirmation once (politely).
        <div class="toast-region">
            <div role="status" aria-live="polite">{move || app.toast.get().filter(|t| !t.error).map(|t| toast_view(app, t))}</div>
            <div role="alert">{move || app.toast.get().filter(|t| t.error).map(|t| toast_view(app, t))}</div>
        </div>
    }
}

fn toast_view(app: App, t: crate::state::Toast) -> impl IntoView {
    view! {
        <div class="toast" class:error=t.error>
            <Icon name=if t.error { "alert" } else { "check" } />
            <span>{t.message}</span>
            <button class="icon-btn small" aria-label="Dismiss" on:click=move |_| app.toast.set(None)><Icon name="close" size=18 /></button>
        </div>
    }
}

#[component]
pub fn BusyOverlay() -> impl IntoView {
    let app = use_app();
    move || app.busy.get().map(|m| view! {
        <div class="busy" role="alert" aria-busy="true">
            <div class="spinner" aria-hidden="true"></div>
            <p>{m}</p>
        </div>
    })
}

/// Clear marker that the visible data is simulated.
#[component]
pub fn DemoBanner() -> impl IntoView {
    let app = use_app();
    move || (app.db.get() == DbKind::Demo).then(|| if app.route.get() == Route::Today {
        view! {
            <div class="demo-banner" role="note">
                <span><strong>"Demo data."</strong>" Everything here is simulated and kept apart from your own records."</span>
                <button class="link" on:click=move |_| app.go(Route::Data)>"Manage"</button>
            </div>
        }.into_any()
    } else {
        view! {
            <div class="demo-banner compact" role="note">
                <span><strong>"Demo data"</strong>" · simulated"</span>
                <button class="link" on:click=move |_| app.go(Route::Data)>"Manage"</button>
            </div>
        }.into_any()
    })
}

/// A labelled card section.
#[component]
pub fn Section(#[prop(into)] title: String, #[prop(optional, into)] action: Option<String>, #[prop(optional)] on_action: Option<Callback<()>>, children: Children) -> impl IntoView {
    view! {
        <section class="section">
            <div class="section-head">
                <h2>{title}</h2>
                {action.zip(on_action).map(|(a, f)| view! { <button class="link" on:click=move |_| f.run(())>{a}</button> })}
            </div>
            {children()}
        </section>
    }
}

/// Width available for charts, so SVG text stays at its intended size.
pub fn chart_width(vw: f64) -> f64 {
    let rail = vw >= 900.0 || crate::platform::media_matches("(min-width: 700px) and (orientation: landscape) and (max-height: 520px)");
    let content = if rail { (vw - 260.0).min(860.0) } else { vw };
    (content - 64.0).clamp(260.0, 760.0)
}

fn kind_label(k: airspace_core::SampleKind) -> &'static str {
    use airspace_core::SampleKind::*;
    match k {
        Instant => "reading",
        Interval => "interval count",
        CumulativeCounter => "running counter",
        DailyTotal => "daily total",
        Session => "session",
    }
}

fn offset_label(m: Option<i32>) -> String {
    match m {
        Some(o) => format!("UTC{}{:02}:{:02}", if o < 0 { "−" } else { "+" }, o.abs() / 60, o.abs() % 60),
        None => "offset not recorded".into(),
    }
}

/// Individual stored records behind a day's value, with full provenance.
#[component]
fn RecordList(metric: airspace_core::Metric, day: chrono::NaiveDate) -> impl IntoView {
    let app = use_app();
    let shown = RwSignal::new(false);
    let rows = RwSignal::new(None::<Vec<airspace_core::Measurement>>);
    // One day's records are small: load them up front so the button can say
    // how many there are, including replaced originals kept for the record.
    leptos::task::spawn_local(async move {
        let mut v = crate::state::load_range_all(app, metric, day, day).await;
        v.sort_by_key(|m| m.start);
        let _ = rows.try_set(Some(v));
    });
    let label = move || match rows.get() {
        None => "Show individual records".to_string(),
        Some(v) => {
            let replaced = v.iter().filter(|m| m.superseded_by.is_some()).count();
            let n = crate::ui::format::count(v.len());
            let noun = if v.len() == 1 { "record" } else { "records" };
            if replaced > 0 { format!("Show {n} individual {noun} ({replaced} replaced, kept)") } else { format!("Show {n} individual {noun}") }
        }
    };
    view! {
        {move || if !shown.get() {
            view! { <button class="link" on:click=move |_| shown.set(true)>{label}</button> }.into_any()
        } else {
            match rows.get() {
                None => view! { <p class="muted" aria-busy="true">"Loading…"</p> }.into_any(),
                Some(v) => {
                    let tz = app.tz_untracked();
                    let s = app.settings.get_untracked();
                    let total = v.len();
                    let limit = 120;
                    let start = total.saturating_sub(limit);
                    let items: Vec<(String, String, String, String)> = v[start..].iter().rev().map(|m| {
                        let when = match m.end {
                            Some(e) if m.kind != airspace_core::SampleKind::Instant && m.kind != airspace_core::SampleKind::CumulativeCounter =>
                                format!("{}–{}", time::format_ts(m.start, &tz, "%H:%M"), time::format_ts(e, &tz, "%H:%M")),
                            _ => time::format_ts(m.start, &tz, "%H:%M:%S"),
                        };
                        let value = if m.metric == airspace_core::Metric::Sleep { time::fmt_minutes(m.value) } else { airspace_core::settings::display(m.metric, m.value, &s).joined() };
                        let kind = format!("{} · {} · {}", kind_label(m.kind), origin_label(m.prov.origin), format!("{:?}", m.prov.status).to_lowercase());
                        let mut details = vec![offset_label(m.utc_offset_min), m.prov.source_name.clone()];
                        match &m.detail {
                            Some(airspace_core::Detail::Hrv { method }) => details.insert(0, format!("method {}", method.label())),
                            Some(airspace_core::Detail::Calories { scope }) => details.insert(0, format!("{} calories", format!("{scope:?}").to_lowercase())),
                            _ => {}
                        }
                        if let Some(d) = &m.prov.device { details.push(d.clone()); }
                        if let Some(id) = &m.prov.original_id { details.push(format!("source id {id}")); }
                        if let Some(b) = &m.prov.batch_id { details.push(format!("batch {b}")); }
                        details.push(format!("stored {}", time::format_ts(m.prov.ingested_at, &tz, "%-d %b %H:%M")));
                        if m.prov.provisional { details.push("provisional mapping".into()); }
                        if let Some(c) = m.prov.confidence { details.push(format!("confidence {c}")); }
                        if let Some(x) = &m.superseded_by { details.push(format!("superseded by {x} — kept, not used in daily values")); }
                        (when, value, kind, details.join(" · "))
                    }).collect();
                    view! {
                        <h3 class="sub">"Stored records"</h3>
                        {(total > limit).then(|| view! { <p class="muted">{format!("Showing the latest {limit} of {total} records. Export CSV for all of them.")}</p> })}
                        <ul class="records" aria-label="Stored records, newest first">
                            {items.into_iter().map(|(w, val, k, d)| view! {
                                <li>
                                    <p class="rec-main tabular"><span>{w}</span><strong>{val}</strong></p>
                                    <p class="rec-kind">{k}</p>
                                    <p class="rec-prov">{d}</p>
                                </li>
                            }).collect_view()}
                        </ul>
                        <p class="muted">"Records are shown exactly as stored; daily values are calculated from them and never replace them."</p>
                    }.into_any()
                }
            }
        }}
    }
}
