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
    view! {
        <div class="row switch-row">
            <div class="row-text">
                <span class="row-title">{label.clone()}</span>
                {hint.map(|h| view! { <span class="row-hint">{h}</span> })}
            </div>
            <button
                role="switch"
                class="switch"
                aria-checked=move || if checked.get() { "true" } else { "false" }
                aria-label=label
                disabled=dis
                on:click=move |_| on_change.run(!checked.get_untracked())
            ><span class="knob"></span></button>
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

/// Global sheet host (provenance and status explanations).
#[component]
pub fn SheetHost() -> impl IntoView {
    let app = use_app();
    let close = Callback::new(move |_| app.sheet.set(None));
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
                                    <dt>"Records"</dt><dd class="tabular">{s.count.to_string()}</dd>
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
        <div class="toast-region" role="status" aria-live="polite">
            {move || app.toast.get().map(|t| view! {
                <div class="toast" class:error=t.error>
                    <Icon name=if t.error { "alert" } else { "check" } />
                    <span>{t.message}</span>
                    <button class="icon-btn small" aria-label="Dismiss" on:click=move |_| app.toast.set(None)><Icon name="close" size=18 /></button>
                </div>
            })}
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
    move || (app.db.get() == DbKind::Demo).then(|| view! {
        <div class="demo-banner" role="note">
            <span><strong>"Demo data."</strong>" Everything here is simulated and kept apart from your own records."</span>
            <button class="link" on:click=move |_| app.go(Route::Data)>"Manage"</button>
        </div>
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
    let content = if vw >= 900.0 { (vw - 260.0).min(860.0) } else { vw };
    (content - 64.0).clamp(260.0, 760.0)
}
