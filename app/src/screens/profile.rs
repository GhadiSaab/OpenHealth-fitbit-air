//! Profile area: device status, data management, import, settings, about.

use crate::platform;
use crate::state::Route;
use crate::ui::charts::DataTable;
use crate::ui::format::plural;
use crate::ui::icons::Icon;
use crate::ui::widgets::{DemoBanner, Header, Section, Segmented, Switch, use_app};
use airspace_core::backup;
use airspace_core::bluetooth::{ConnectionState, LiveProvider, NotConnectedLive, Support, live_capabilities};
use airspace_core::catalog;
use airspace_core::dedupe::{ConflictPolicy, plan_merge};
use airspace_core::import::{self, ImportContext, ImportPreview, ParsedImport, Severity};
use airspace_core::providers::{ProviderStatus, descriptors};
use airspace_core::settings::{MotionPref, TempUnit, Theme, UnitSystem};
use airspace_core::time;
use airspace_core::{DbKind, ImportBatch, Measurement, Metric};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;
use wasm_bindgen::JsCast;

#[component]
pub fn Profile() -> impl IntoView {
    let app = use_app();
    let row = move |icon: &'static str, title: &'static str, hint: &'static str, r: Route| {
        view! {
            <li><button class="nav-row" on:click=move |_| app.go(r)>
                <Icon name=icon />
                <span class="row-text"><span class="row-title">{title}</span><span class="row-hint">{hint}</span></span>
                <Icon name="right" size=18 />
            </button></li>
        }
    };
    view! {
        <Header title="Profile & device" back=true />
        <DemoBanner />
        <DeviceCard />
        <ul class="nav-list">
            {row("bluetooth", "Device", "Connection, capabilities and recording", Route::Device)}
            {row("database", "Data", "Import, export, backups and deletion", Route::Data)}
            {row("settings", "Settings", "Units, theme, time zone, motion", Route::Settings)}
            {row("coach", "AI provider", "Optional; your own key", Route::AiSettings)}
            {row("info", "About Airspace", "Privacy and what this version does", Route::About)}
        </ul>
    }
}

#[component]
fn DeviceCard() -> impl IntoView {
    let app = use_app();
    move || {
        let now = app.now.get();
        match (app.db.get(), app.demo_device.get()) {
            (DbKind::Demo, Some(d)) => view! {
                <div class="device-card">
                    <span class="status-dot warn" aria-hidden="true"></span>
                    <div><p class="row-title">{d.name.clone()}</p>
                    <p class="row-hint">{format!("Simulated · disconnected {} · battery {}%", time::fmt_ago(d.disconnected_at, now), d.battery_pct)}</p></div>
                </div>
            }.into_any(),
            _ => view! {
                <div class="device-card">
                    <span class="status-dot off" aria-hidden="true"></span>
                    <div><p class="row-title">"Fitbit Air"</p><p class="row-hint">"Not connected · Bluetooth recording isn't in this version"</p></div>
                </div>
            }.into_any(),
        }
    }
}

#[component]
pub fn Device() -> impl IntoView {
    let app = use_app();
    let provider = NotConnectedLive;
    let state = provider.state();
    let reason = match &state {
        ConnectionState::Unavailable { reason } => reason.clone(),
        _ => String::new(),
    };
    let previews: Vec<(ConnectionState, &str)> = vec![
        (ConnectionState::PermissionDenied { can_ask_again: true }, "Airspace needs the Nearby devices permission to talk to your tracker. You'll be asked once, with an explanation."),
        (ConnectionState::BluetoothOff, "Turn on Bluetooth to record. History stays available."),
        (ConnectionState::OutOfRange { since: 0 }, "The tracker is too far away. Readings pause and the gap is recorded."),
        (ConnectionState::Reconnecting { attempt: 2, since: 0 }, "Connection dropped; retrying. Readings received before the drop are kept."),
        (ConnectionState::AuthorizationExpired { message: String::new() }, "The tracker's credentials need renewing. Live recording stops until then; nothing is deleted."),
    ];
    view! {
        <Header title="Device" back=true />
        <Section title="Connection">
            <div class="device-card">
                <span class="status-dot off" aria-hidden="true"></span>
                <div><p class="row-title">{state.title()}</p><p class="row-hint">{reason}</p></div>
            </div>
            <button class="btn" disabled=true aria-describedby="connect-why">"Connect a tracker"</button>
            <p id="connect-why" class="muted">"Pairing isn't available yet. Airspace won't pretend to connect: live recording will arrive as a native Android service once its data mapping is validated."</p>
            {move || (app.db.get() == DbKind::Demo).then(|| view! {
                <p class="notice">"The demo includes a simulated recording session that ended with a disconnection, so you can see how stale readings and gaps look."</p>
            })}
        </Section>
        <Section title="What live recording will provide">
            <DataTable caption="Live Bluetooth capabilities" headers=vec!["Measurement", "From Bluetooth"]
                rows=live_capabilities().into_iter().map(|(m, s)| vec![catalog::info(m).title.to_string(), match s {
                    Support::Supported => "Yes".into(),
                    Support::Provisional => "Demonstrated; mapping to be validated".into(),
                    Support::Unsupported => "No — history import only".into(),
                }]).collect() />
            <p class="muted">"Sleep stages, SpO₂, HRV, respiratory rate and temperature are never inferred from the live stream."</p>
        </Section>
        <Section title="States you may see">
            <ul class="state-previews">
                {previews.into_iter().map(|(s, d)| view! {
                    <li><span class="status-dot warn" aria-hidden="true"></span><div><p class="row-title">{s.title()}</p><p class="row-hint">{d}</p></div></li>
                }).collect_view()}
            </ul>
        </Section>
        <Section title="Data sources">
            <ul class="provider-list">
                {move || descriptors(app.ai.get().is_some()).into_iter().map(|p| {
                    let (cls, txt) = match &p.status {
                        ProviderStatus::Available => ("ok", "Available".to_string()),
                        ProviderStatus::Connected => ("ok", "Connected".to_string()),
                        ProviderStatus::NotConnected { reason } => ("off", format!("Not connected — {reason}")),
                        ProviderStatus::NeedsSetup { what } => ("warn", format!("Not set up — {what}")),
                    };
                    view! { <li><span class=format!("status-dot {cls}") aria-hidden="true"></span><div><p class="row-title">{p.name}</p><p class="row-hint">{p.description}</p><p class="row-hint">{txt}</p></div></li> }
                }).collect_view()}
            </ul>
        </Section>
    }
}

#[component]
pub fn DataScreen() -> impl IntoView {
    let app = use_app();
    let db_sig = Signal::derive(move || app.db.get());
    let switch = Callback::new(move |k: DbKind| {
        spawn_local(async move {
            let r = if k == DbKind::Demo { app.open_demo().await } else { app.switch_db(k).await };
            match r {
                Ok(()) => app.notify(if k == DbKind::Demo { "Showing demo data" } else { "Showing your personal data" }),
                Err(e) => app.error(e),
            }
        });
    });
    let csv_metric = RwSignal::new(Metric::Steps);
    let confirm_delete = RwSignal::new(false);
    let export_json = move || {
        spawn_local(async move {
            app.busy.set(Some("Preparing backup…".into()));
            let r = async {
                let json = app.backup_json().await?;
                let name = format!("airspace-{}-backup-{}.json", app.db.get_untracked().key(), time::day_key(app.today_untracked()));
                platform::save_file(&name, "application/json", &json).await
            }
            .await;
            app.busy.set(None);
            match r {
                Ok(n) => app.notify(format!("Backup saved: {n}")),
                Err(e) => app.error(e),
            }
        });
    };
    let export_csv = move || {
        let m = csv_metric.get_untracked();
        spawn_local(async move {
            let r = async {
                let csv = app.metric_csv(m).await?;
                platform::save_file(&format!("airspace-{}-{}.csv", m.key(), time::day_key(app.today_untracked())), "text/csv", &csv).await
            }
            .await;
            match r {
                Ok(n) => app.notify(format!("Exported {n}")),
                Err(e) => app.error(e),
            }
        });
    };
    let delete = move || {
        let k = app.db.get_untracked();
        spawn_local(async move {
            match app.delete_all(k).await {
                Ok(()) => {
                    confirm_delete.set(false);
                    app.haptic();
                    app.notify(if k == DbKind::Demo { "Demo data deleted" } else { "All personal data deleted from this device" });
                }
                Err(e) => app.error(e),
            }
        });
    };
    view! {
        <Header title="Data" back=true />
        <Section title="Active data">
            <Segmented options=vec![(DbKind::Personal, "My data"), (DbKind::Demo, "Demo")] value=db_sig on_change=switch label="Active database" />
            <p class="muted">"Demo data and your personal data are stored in separate databases and never mixed. Imports always go to your personal data."</p>
            <p class="tabular">{move || format!("{} stored · {} journal entries", plural(app.measurement_count.get() as usize, "measurement", "measurements"), app.journal.with(|j| j.len()))}</p>
            {move || (app.db.get() == DbKind::Demo).then(|| view! {
                <button class="btn quiet" on:click=move |_| spawn_local(async move {
                    match app.generate_demo().await { Ok(()) => { let _ = app.load().await; app.notify("Demo regenerated") } Err(e) => app.error(e) }
                })><Icon name="refresh" size=18 />"Regenerate demo"</button>
            })}
        </Section>
        <Section title="Import">
            <p class="muted">"Bring in history from a file in the Airspace import format. You'll see a preview with any problems before anything is saved."</p>
            <button class="btn primary" on:click=move |_| app.go(Route::Import)><Icon name="upload" size=18 />"Import history"</button>
            <RestoreBackup />
        </Section>
        <Section title="Export">
            <p class="muted">"A backup contains every measurement with its provenance, your journal and settings. It never contains your AI key."</p>
            <button class="btn" on:click=move |_| export_json()><Icon name="download" size=18 />"Export full backup (JSON)"</button>
            <div class="inline-add">
                <label class="sr-only" for="csv-metric">"Metric to export"</label>
                <select id="csv-metric" on:change=move |ev| { if let Some(m) = Metric::from_key(&event_target_value(&ev)) { csv_metric.set(m) } }>
                    {Metric::ALL.into_iter().map(|m| view! { <option value=m.key() selected=move || csv_metric.get() == m>{catalog::info(m).title}</option> }).collect_view()}
                </select>
                <button class="btn" on:click=move |_| export_csv()>"Export CSV"</button>
            </div>
        </Section>
        <ImportHistory />
        <Section title="Maintenance">
            <button class="btn quiet" on:click=move |_| spawn_local(async move { match app.recompute_all().await { Ok(()) => app.notify("Daily summaries recalculated"), Err(e) => app.error(e) } })>
                <Icon name="refresh" size=18 />"Recalculate daily summaries"</button>
            <p class="muted">"Summaries are derived from the stored measurements and can always be rebuilt. Original measurements are never changed."</p>
        </Section>
        <Section title="Delete">
            {move || if confirm_delete.get() {
                view! {
                    <div class="confirm" role="alertdialog" aria-labelledby="del-title">
                        <p id="del-title"><strong>{move || if app.db.get() == DbKind::Demo { "Delete all demo data?" } else { "Delete all personal data on this device?" }}</strong></p>
                        <p>"Measurements, summaries, journal entries and import history will be removed. This can't be undone. Export a backup first if you might need it."</p>
                        <div class="btn-row">
                            <button class="btn danger" on:click=move |_| delete()>"Delete everything"</button>
                            <button class="btn quiet" on:click=move |_| confirm_delete.set(false)>"Cancel"</button>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! { <button class="btn danger-outline" on:click=move |_| confirm_delete.set(true)><Icon name="trash" size=18 />{move || if app.db.get() == DbKind::Demo { "Delete demo data" } else { "Delete all my data" }}</button> }.into_any()
            }}
        </Section>
    }
}

#[component]
fn ImportHistory() -> impl IntoView {
    let app = use_app();
    move || {
        let b = app.batches.get();
        (!b.is_empty()).then(|| {
            let mut b = b;
            b.sort_by(|a, c| c.created_at.cmp(&a.created_at));
            let tz = app.tz();
            let rows = b.iter().map(|x| vec![
                time::format_ts(x.created_at, &tz, "%-d %b %Y %H:%M"),
                x.file_name.clone(),
                format!("{} added, {} duplicates, {} conflicts ({})", x.added, x.duplicates, x.conflicts, x.conflict_policy.replace('_', " ")),
                match (&x.first_day, &x.last_day) { (Some(a), Some(c)) => format!("{a} – {c}"), _ => String::new() },
            ]).collect();
            view! { <Section title="Import history"><DataTable caption="Previous imports" headers=vec!["When", "File", "Result", "Dates"] rows=rows /></Section> }
        })
    }
}

fn file_from(ev: &leptos::ev::Event) -> Option<web_sys::File> {
    let input: web_sys::HtmlInputElement = ev.target()?.dyn_into().ok()?;
    input.files()?.get(0)
}

#[component]
fn RestoreBackup() -> impl IntoView {
    let app = use_app();
    let parsed = RwSignal::new(None::<Result<backup::Backup, String>>);
    let on_file = move |ev: leptos::ev::Event| {
        let Some(f) = file_from(&ev) else { return };
        spawn_local(async move {
            let r = match platform::read_file(&f).await {
                Ok(text) => backup::parse_backup(&text).map_err(|e| e.to_string()),
                Err(e) => Err(e),
            };
            parsed.set(Some(r));
        });
    };
    let restore = move |replace: bool| {
        let Some(Ok(b)) = parsed.get_untracked() else { return };
        spawn_local(async move {
            app.busy.set(Some("Restoring backup…".into()));
            let r = app.restore(b, replace).await;
            app.busy.set(None);
            match r {
                Ok(n) => {
                    parsed.set(None);
                    app.haptic();
                    app.notify(format!("Restored {} new measurements", n));
                }
                Err(e) => app.error(e),
            }
        });
    };
    view! {
        <label class="btn quiet file-btn">
            <Icon name="refresh" size=18 />"Restore a backup…"
            <input type="file" accept=".json,application/json" class="sr-only" on:change=on_file />
        </label>
        {move || parsed.get().map(|r| match r {
            Err(e) => view! { <p class="error-text" role="alert">{e}</p> }.into_any(),
            Ok(b) => {
                let tz = app.tz_untracked();
                view! {
                    <div class="confirm">
                        <p><strong>"Backup ready to restore"</strong></p>
                        <p class="tabular">{format!("{} database · created {} · {} measurements · {} journal entries · format v{}", if b.database == DbKind::Demo { "Demo" } else { "Personal" }, time::format_ts(b.created_at, &tz, "%-d %b %Y %H:%M"), b.measurements.len(), b.journal.len(), b.version)}</p>
                        <p class="muted">"Merge adds anything not already stored (matching records are skipped). Replace deletes the current data in this database first."</p>
                        <div class="btn-row">
                            <button class="btn primary" on:click=move |_| restore(false)>"Merge"</button>
                            <button class="btn danger-outline" on:click=move |_| restore(true)>"Replace"</button>
                            <button class="btn quiet" on:click=move |_| parsed.set(None)>"Cancel"</button>
                        </div>
                    </div>
                }.into_any()
            }
        })}
    }
}

#[derive(Clone)]
struct Staged {
    parsed: ParsedImport,
    preview: ImportPreview,
    existing: Vec<Measurement>,
    file_name: String,
}

#[component]
pub fn ImportScreen() -> impl IntoView {
    let app = use_app();
    let staged = StoredValue::new_local(None::<Staged>);
    let ver = RwSignal::new(0u32);
    let policy = RwSignal::new(ConflictPolicy::KeepExisting);
    let error = RwSignal::new(None::<String>);
    let working = RwSignal::new(false);
    let on_file = move |ev: leptos::ev::Event| {
        let Some(f) = file_from(&ev) else { return };
        let name = f.name();
        error.set(None);
        working.set(true);
        spawn_local(async move {
            // Imports always go to personal data.
            if app.db.get_untracked() == DbKind::Demo {
                if let Err(e) = app.switch_db(DbKind::Personal).await {
                    error.set(Some(e));
                    working.set(false);
                    return;
                }
                app.notify("Switched to your personal data for this import.");
            }
            let text = match platform::read_file(&f).await {
                Ok(t) => t,
                Err(e) => {
                    error.set(Some(e));
                    working.set(false);
                    return;
                }
            };
            if backup::is_backup(&text) {
                error.set(Some("This is an Airspace backup. Use “Restore a backup” on the Data screen instead.".into()));
                working.set(false);
                return;
            }
            let now = platform::now_ms();
            let ctx = ImportContext { default_tz: app.tz_untracked(), now, batch_id: format!("imp-{now}"), file_name: name.clone() };
            let parsed = import::parse_file(&name, &text, &ctx);
            let existing = match import::span(&parsed.records) {
                Some((a, b)) => crate::repo::measurements(DbKind::Personal, None, a - 2 * time::DAY_MS, b + 2 * time::DAY_MS).await.unwrap_or_default(),
                None => vec![],
            };
            let preview = import::preview(&parsed, &existing, &ctx.default_tz);
            staged.set_value(Some(Staged { parsed, preview, existing, file_name: name }));
            ver.update(|v| *v += 1);
            working.set(false);
        });
    };
    let commit = move || {
        let Some(st) = staged.get_value() else { return };
        let pol = policy.get_untracked();
        working.set(true);
        spawn_local(async move {
            let plan = plan_merge(st.parsed.records.clone(), &st.preview.classes, pol);
            let mut writes = plan.insert.clone();
            // Never overwrite: superseded originals are kept, only flagged.
            let by_id: HashMap<&str, &Measurement> = st.existing.iter().map(|m| (m.id.as_str(), m)).collect();
            for (old, new) in &plan.supersede {
                if let Some(m) = by_id.get(old.as_str()) {
                    let mut m = (*m).clone();
                    m.superseded_by = Some(new.clone());
                    writes.push(m);
                }
            }
            let batch_id = st.parsed.records.first().and_then(|m| m.prov.batch_id.clone()).unwrap_or_else(|| format!("imp-{}", platform::now_ms()));
            let r = async {
                app.busy.set(Some(format!("Importing {}…", plural(plan.insert.len(), "record", "records"))));
                app.ingest(writes).await?;
                let mut j = st.parsed.journal.clone();
                // Imported notes never replace notes already written for that day.
                let have: Vec<String> = app.journal.with_untracked(|x| x.iter().map(|e| e.day.clone()).collect());
                j.retain(|e| !have.contains(&e.day));
                for e in j {
                    app.save_journal(e).await?;
                }
                let t = st.preview.totals();
                crate::repo::put_batch(DbKind::Personal, &ImportBatch {
                    id: batch_id,
                    created_at: platform::now_ms(),
                    adapter: st.parsed.adapter.clone(),
                    file_name: st.file_name.clone(),
                    added: plan.insert.len(),
                    duplicates: t.duplicates,
                    conflicts: t.conflicts,
                    conflict_policy: pol.key().into(),
                    first_day: st.preview.first_day.clone(),
                    last_day: st.preview.last_day.clone(),
                }).await?;
                app.batches.set(crate::repo::batches(DbKind::Personal).await.unwrap_or_default());
                Ok::<usize, String>(plan.insert.len())
            }
            .await;
            app.busy.set(None);
            working.set(false);
            match r {
                Ok(n) => {
                    app.haptic();
                    app.notify(format!("Imported {}", plural(n, "record", "records")));
                    app.update_settings(|s| s.onboarded = true);
                    staged.set_value(None);
                    ver.update(|v| *v += 1);
                    if let Some(d) = app.summaries.with_untracked(|i| i.last_day()) {
                        app.day.set(d.min(app.today_untracked()));
                    }
                    app.go(Route::Today);
                }
                Err(e) => app.error(format!("Import failed: {e}")),
            }
        });
    };
    view! {
        <Header title="Import history" back=true />
        <Section title="Choose a file">
            <p>"Airspace reads its documented import format as JSON or CSV. Other exports need converting first — see the import guide in the project documentation."</p>
            <label class="btn primary file-btn">
                <Icon name="upload" size=18 />"Choose file…"
                <input type="file" accept=".json,.csv,application/json,text/csv" class="sr-only" on:change=on_file />
            </label>
            {move || working.get().then(|| view! { <p class="muted" aria-busy="true">"Reading…"</p> })}
            {move || error.get().map(|e| view! { <p class="error-text" role="alert">{e}</p> })}
        </Section>
        {move || {
            ver.get();
            staged.with_value(|s| s.clone()).map(|st| {
                let t = st.preview.totals();
                let tz = app.tz_untracked();
                let rows: Vec<Vec<String>> = st.preview.per_metric.iter().map(|(m, c)| vec![
                    catalog::info(*m).title.to_string(),
                    c.new.to_string(),
                    c.duplicates.to_string(),
                    c.conflicts.to_string(),
                    match (c.first, c.last) { (Some(a), Some(b)) => format!("{} – {}", time::format_ts(a, &tz, "%-d %b %Y"), time::format_ts(b, &tz, "%-d %b %Y")), _ => String::new() },
                ]).collect();
                let errors = st.parsed.errors();
                let warnings = st.parsed.warnings();
                let issues: Vec<_> = st.parsed.issues.iter().take(100).cloned().collect();
                let more = st.parsed.issues.len().saturating_sub(100);
                let nothing = t.new == 0 && (t.conflicts == 0 || policy.get() == ConflictPolicy::KeepExisting);
                view! {
                    <Section title="Preview">
                        <p class="tabular">{format!("{} · {} · {}", st.file_name, st.parsed.source_name, st.parsed.adapter)}</p>
                        {st.preview.first_day.clone().map(|a| view! { <p class="tabular">{format!("Dates: {a} to {} ({})", st.preview.last_day.clone().unwrap_or_default(), plural(st.preview.days, "day", "days"))}</p> })}
                        <p class="tabular"><strong>{format!("{} new", t.new)}</strong>{format!(" · {} already stored · {} conflicting · {} rejected · {} warnings", t.duplicates, t.conflicts, errors, warnings)}</p>
                        {(!rows.is_empty()).then(|| view! { <DataTable caption="Records by metric" headers=vec!["Metric", "New", "Duplicates", "Conflicts", "Dates"] rows=rows /> })}
                        {(!st.parsed.journal.is_empty()).then(|| view! { <p>{format!("{} — added only for days without an existing entry.", plural(st.parsed.journal.len(), "journal entry", "journal entries"))}</p> })}
                        {(t.conflicts > 0).then(|| view! {
                            <fieldset class="policy">
                                <legend class="field-label">{format!("{} records conflict with stored values for the same time. What should happen?", t.conflicts)}</legend>
                                {[(ConflictPolicy::KeepExisting, "Keep what's stored", "Skip the conflicting records from this file."),
                                  (ConflictPolicy::PreferIncoming, "Prefer this file", "Use the new values; stored originals are kept and marked as superseded."),
                                  (ConflictPolicy::KeepBoth, "Keep both", "Store both. Daily totals still avoid double counting by source rules.")].into_iter().map(|(p, l, h)| view! {
                                    <label class="check-row radio">
                                        <input type="radio" name="policy" prop:checked=move || policy.get() == p on:change=move |_| policy.set(p) />
                                        <span><span class="row-title">{l}</span><span class="row-hint">{h}</span></span>
                                    </label>
                                }).collect_view()}
                            </fieldset>
                        })}
                        {(!issues.is_empty()).then(|| view! {
                            <details class="issues" open={errors > 0}>
                                <summary>{format!("Validation problems ({})", st.parsed.issues.len())}</summary>
                                <ul class="plain">
                                    {issues.into_iter().map(|i| view! {
                                        <li class=if i.severity == Severity::Error { "issue err" } else { "issue warn" }>
                                            <span class="pill">{if i.severity == Severity::Error { "Skipped" } else { "Note" }}</span>
                                            {if i.line > 0 { format!("Record {}: {}", i.line, i.message) } else { i.message.clone() }}
                                        </li>
                                    }).collect_view()}
                                    {(more > 0).then(|| view! { <li>{format!("…and {more} more")}</li> })}
                                </ul>
                            </details>
                        })}
                        <div class="btn-row">
                            <button class="btn primary" disabled=move || nothing || working.get() on:click=move |_| commit()>
                                {if nothing { "Nothing new to import".to_string() } else { format!("Import {}", plural(t.new + if policy.get() != ConflictPolicy::KeepExisting { t.conflicts } else { 0 }, "record", "records")) }}
                            </button>
                            <button class="btn quiet" on:click=move |_| { staged.set_value(None); ver.update(|v| *v += 1); }>"Cancel"</button>
                        </div>
                    </Section>
                }
            })
        }}
        <Section title="Format">
            <p class="muted">"JSON files start with "<code>"{\"schema\": \"airspace.import\", \"version\": 1}"</code>" and list records with a metric, time and value. CSV files use the same field names as columns. Times without an offset are read in the file's time zone (or yours)."</p>
            <pre class="json small" tabindex="0">{IMPORT_EXAMPLE}</pre>
        </Section>
    }
}

const IMPORT_EXAMPLE: &str = r#"{
  "schema": "airspace.import", "version": 1,
  "source": {"name": "My export", "device": "Fitbit Air"},
  "timezone": "Europe/Paris",
  "records": [
    {"metric": "heart_rate", "time": "2026-10-01T08:00:00+02:00", "value": 62},
    {"metric": "steps", "kind": "daily_total", "day": "2026-10-01", "value": 9120},
    {"metric": "hrv", "time": "2026-10-01T07:02:00+02:00", "value": 41, "method": "rmssd"},
    {"metric": "sleep", "start": "2026-09-30T23:20:00+02:00",
     "end": "2026-10-01T07:02:00+02:00", "stages": [ … ]}
  ]
}"#;

#[component]
pub fn SettingsScreen() -> impl IntoView {
    let app = use_app();
    let theme = Signal::derive(move || app.settings.with(|s| s.theme));
    let units = Signal::derive(move || app.settings.with(|s| s.units));
    let temp = Signal::derive(move || app.settings.with(|s| s.temperature));
    let zones: Vec<&'static str> = chrono_tz::TZ_VARIANTS.iter().map(|z| z.name()).filter(|n| n.contains('/') || *n == "UTC").collect();
    let change_tz = move |tz: String| {
        app.update_settings(|s| s.timezone = tz);
        spawn_local(async move {
            match app.recompute_all().await {
                Ok(()) => app.notify("Time zone updated; daily summaries recalculated"),
                Err(e) => app.error(e),
            }
        });
    };
    view! {
        <Header title="Settings" back=true />
        <Section title="Appearance">
            <Segmented options=vec![(Theme::System, "System"), (Theme::Light, "Light"), (Theme::Dark, "Dark")] value=theme
                on_change=Callback::new(move |t| app.update_settings(|s| s.theme = t)) label="Theme" />
            <Switch label="Reduce motion" hint="Also follows your system setting."
                checked=Signal::derive(move || app.settings.with(|s| s.motion == MotionPref::Reduced))
                on_change=Callback::new(move |v: bool| app.update_settings(|s| s.motion = if v { MotionPref::Reduced } else { MotionPref::System })) />
            {platform::is_android().then(|| view! {
                <Switch label="Haptic feedback" hint="A light tick when you save or import."
                    checked=Signal::derive(move || app.settings.with(|s| s.haptics))
                    on_change=Callback::new(move |v| app.update_settings(|s| s.haptics = v)) />
            })}
        </Section>
        <Section title="Units">
            <p class="field-label">"Distance"</p>
            <Segmented options=vec![(UnitSystem::Metric, "Kilometres"), (UnitSystem::Imperial, "Miles")] value=units
                on_change=Callback::new(move |u| app.update_settings(|s| s.units = u)) label="Distance units" />
            <p class="field-label">"Temperature deviation"</p>
            <Segmented options=vec![(TempUnit::Celsius, "°C"), (TempUnit::Fahrenheit, "°F")] value=temp
                on_change=Callback::new(move |u| app.update_settings(|s| s.temperature = u)) label="Temperature units" />
        </Section>
        <Section title="Time zone">
            <p class="muted">"Used to decide which day a measurement belongs to when the source didn't record its own offset. Changing it recalculates daily summaries."</p>
            <label class="field"><span class="field-label">"Home time zone"</span>
                <select on:change=move |ev| change_tz(event_target_value(&ev))>
                    {zones.into_iter().map(|z| view! { <option value=z selected=move || app.settings.with(|s| s.timezone == z)>{z.replace('_', " ")}</option> }).collect_view()}
                </select>
            </label>
            <button class="link" on:click=move |_| change_tz(platform::detect_timezone())>{format!("Use this device's zone ({})", platform::detect_timezone())}</button>
        </Section>
    }
}

#[component]
pub fn About() -> impl IntoView {
    let rows: Vec<Vec<String>> = vec![
        vec!["Today, Explore, Journal, local insights".into(), "Working".into()],
        vec!["Local storage (IndexedDB / SQLite), backup, CSV export".into(), "Working".into()],
        vec!["History import (JSON/CSV) with preview, duplicates, conflicts".into(), "Working".into()],
        vec!["Demo dataset".into(), "Simulated — clearly marked, separate database".into()],
        vec!["AI coach".into(), "Optional — works only with your own provider and key".into()],
        vec!["Bluetooth live recording".into(), "Not in this version — interface prepared".into()],
        vec!["Google-backed history sync".into(), "Not in this version — reports Not connected".into()],
    ];
    view! {
        <Header title="About Airspace" back=true />
        <Section title="Privacy">
            <ul class="plain checklist">
                <li>"No account, no analytics, no advertising or tracking SDKs."</li>
                <li>"All data stays on this device unless you export it or explicitly send a summary to your AI provider."</li>
                <li>"API keys are never written to backups, exports, logs or the offline cache."</li>
            </ul>
        </Section>
        <Section title="What this version does">
            <DataTable caption="Feature status" headers=vec!["Feature", "Status"] rows=rows />
        </Section>
        <Section title="Not medical advice">
            <p>"Airspace displays measurements from a consumer wrist tracker. It doesn't diagnose, treat or prevent any condition. Talk to a clinician about health concerns."</p>
        </Section>
        <Section title="Credits">
            <p class="muted">{format!("Version {} · Built with Rust, Leptos and Tauri · Inter typeface (SIL Open Font License).", env!("CARGO_PKG_VERSION"))}</p>
        </Section>
    }
}
