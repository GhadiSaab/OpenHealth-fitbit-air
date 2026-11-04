//! Today: a focused daily overview.

use crate::state::{Range, Route, Sheet, async_signal, load_range};
use crate::ui::charts::{Ring, Sparkline};
use crate::ui::format::{self, day_short, day_title};
use crate::ui::icons::{Icon, metric_icon};
use crate::ui::widgets::{DemoBanner, EmptyState, Header, Section, use_app};
use airspace_core::aggregate::SummaryExtra;
use airspace_core::bluetooth::{Freshness, freshness};
use airspace_core::catalog;
use airspace_core::insights::compare_periods;
use airspace_core::time::{self, HOUR_MS};
use airspace_core::{DbKind, Metric, Origin};
use chrono::Duration;
use leptos::prelude::*;

#[component]
pub fn Today() -> impl IntoView {
    let app = use_app();
    let title = move || day_title(app.day.get(), app.today());
    view! {
        {move || view! { <Header title=title() /> }}
        <DemoBanner />
        <DateNav />
        <StatusStrip />
        {move || {
            let empty = app.summaries.with(|s| s.is_empty());
            if empty {
                view! { <NoData /> }.into_any()
            } else {
                view! {
                    <DaySummaryText />
                    <div class="today-grid">
                        <StepsHero />
                        <HeartTile />
                        <SleepTile />
                        <PairTiles />
                    </div>
                    <Trends />
                    <Favorites />
                }.into_any()
            }
        }}
    }
}

#[component]
fn NoData() -> impl IntoView {
    let app = use_app();
    view! {
        <EmptyState icon="database" title="No data yet" body="Airspace keeps everything on this device. Import your history, or explore the demo to see how it works.">
            <div class="btn-row">
                <button class="btn primary" on:click=move |_| app.go(Route::Import)>"Import history"</button>
                <button class="btn" on:click=move |_| { leptos::task::spawn_local(async move { if let Err(e) = app.open_demo().await { app.error(e) } }); }>"Explore demo"</button>
            </div>
        </EmptyState>
    }
}

#[component]
fn DateNav() -> impl IntoView {
    let app = use_app();
    let is_today = move || app.day.get() >= app.today();
    let first = move || app.summaries.with(|s| s.first_day());
    view! {
        <div class="date-nav" role="group" aria-label="Choose day">
            <button class="icon-btn" aria-label="Previous day" on:click=move |_| { app.day.update(|d| *d -= Duration::days(1)); }>
                <Icon name="left" />
            </button>
            <label class="date-pick">
                <span class="sr-only">"Date"</span>
                <Icon name="calendar" size=18 />
                <input
                    type="date"
                    prop:value=move || time::day_key(app.day.get())
                    max=move || time::day_key(app.today())
                    min=move || first().map(time::day_key)
                    on:change=move |ev| { if let Some(d) = time::parse_day(&event_target_value(&ev)) { if d <= app.today() { app.day.set(d) } } }
                />
                <span class="date-text tabular">{move || day_short(app.day.get())}</span>
            </label>
            <button class="icon-btn" aria-label="Next day" disabled=is_today on:click=move |_| { if !is_today() { app.day.update(|d| *d += Duration::days(1)); } }>
                <Icon name="right" />
            </button>
            {move || (!is_today()).then(|| view! { <button class="chip" on:click=move |_| app.day.set(app.today())>"Today"</button> })}
        </div>
    }
}

/// Connection, recording and freshness at a glance; details on tap.
#[component]
fn StatusStrip() -> impl IntoView {
    let app = use_app();
    let text = move || {
        let now = app.now.get();
        match (app.db.get(), app.demo_device.get()) {
            (DbKind::Demo, Some(d)) => (
                "warn",
                format!("Demo device · disconnected {} · not recording", time::fmt_ago(d.disconnected_at, now)),
            ),
            _ if app.summaries.with(|i| i.is_empty()) => ("off", "Not connected · nothing imported yet".to_string()),
            _ => ("off", "Not connected · showing imported history".to_string()),
        }
    };
    view! {
        <button class="status-strip" on:click=move |_| app.sheet.set(Some(Sheet::Status)) aria-haspopup="dialog">
            <span class=move || format!("status-dot {}", text().0) aria-hidden="true"></span>
            <span>{move || text().1}</span>
            <Icon name="info" size=18 />
        </button>
    }
}

#[component]
fn DaySummaryText() -> impl IntoView {
    let app = use_app();
    let text = move || {
        let day = app.day.get();
        let today = app.today();
        let s = app.settings.get();
        app.summaries.with(|idx| {
            let mut parts = vec![];
            let lead = if day == today { "So far today".to_string() } else { format!("On {}", day.format("%A")) };
            match idx.value(Metric::Steps, day) {
                Some(v) => parts.push(format!(
                    "{lead} you took {} ({:.0}% of your goal)",
                    format::value(Metric::Steps, v, &s),
                    v / s.step_goal.max(1) as f64 * 100.0
                )),
                None => parts.push(format!("{lead} no steps were recorded")),
            }
            if let Some(v) = idx.value(Metric::Sleep, day) {
                parts.push(format!("you slept {}", time::fmt_minutes(v)));
            }
            if let Some(v) = idx.value(Metric::RestingHeartRate, day) {
                parts.push(format!("your resting heart rate was {}", format::value(Metric::RestingHeartRate, v, &s)));
            }
            // "A, B and C."
            let mut out = match parts.len() {
                0 | 1 => parts.join(""),
                n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
            };
            out.push('.');
            if !Metric::ALL.iter().any(|m| idx.get(*m, day).is_some()) {
                out = format!("Nothing was recorded on {}. Missing days are shown as gaps.", day_short(day));
            }
            out
        })
    };
    view! { <p class="day-summary">{text}</p> }
}

#[component]
fn StepsHero() -> impl IntoView {
    let app = use_app();
    let data = move || {
        let d = app.day.get();
        app.summaries.with(|i| i.get(Metric::Steps, d).cloned())
    };
    view! {
        <button class="tile hero tone-activity" on:click=move |_| app.go(Route::Metric(Metric::Steps, Range::Day))>
            {move || {
                let s = app.settings.get();
                let goal = s.step_goal as f64;
                match data().and_then(|x| x.value.map(|v| (v, x.provisional))) {
                    Some((v, prov)) => view! {
                        <Ring fraction=v / goal.max(1.0) tone="activity" size=112.0 />
                        <div class="hero-text">
                            <span class="tile-label"><Icon name="steps" size=18 />"Steps"</span>
                            <span class="hero-value tabular">{format::value(Metric::Steps, v, &s).replace("\u{a0}steps", "")}</span>
                            <span class="tile-sub">{format!("{:.0}% of {} goal", v / goal.max(1.0) * 100.0, format::value(Metric::Steps, goal, &s).replace("\u{a0}steps", ""))}</span>
                            {prov.then(|| view! { <span class="pill prov" title="Includes provisional live values">"Includes provisional values"</span> })}
                        </div>
                    }.into_any(),
                    None => view! {
                        <Ring fraction=0.0 tone="activity" size=112.0 />
                        <div class="hero-text">
                            <span class="tile-label"><Icon name="steps" size=18 />"Steps"</span>
                            <span class="hero-value muted">"—"</span>
                            <span class="tile-sub">"No step data for this day"</span>
                        </div>
                    }.into_any(),
                }
            }}
            <MiniWeek />
        </button>
    }
}

#[component]
fn HeartTile() -> impl IntoView {
    let app = use_app();
    let samples = async_signal(move || (app.db.get(), app.day.get(), app.data_rev.get()), move |(_, d, _)| load_range(app, Metric::HeartRate, d, d));
    view! {
        <button class="tile tone-heart" on:click=move |_| app.go(Route::Metric(Metric::HeartRate, Range::Day))>
            <span class="tile-label"><Icon name="heart" size=18 />"Heart rate"</span>
            {move || {
                let now = app.now.get();
                let tz = app.tz();
                match samples.get() {
                    None => view! { <span class="tile-value muted">"…"</span> }.into_any(),
                    Some(v) => match v.iter().max_by_key(|m| m.start) {
                        None => view! { <span class="tile-value muted">"—"</span><span class="tile-sub">"No readings this day"</span> }.into_any(),
                        Some(last) => {
                            let live = last.prov.origin == Origin::BluetoothLive;
                            let f = freshness(last.start, now, live, false);
                            let (pill, cls) = match f {
                                Freshness::Live => ("Live".to_string(), "live"),
                                Freshness::Recent => (format!("Recent · {}", time::fmt_ago(last.start, now)), "recent"),
                                Freshness::Stale => (format!("Last reading · {}", time::fmt_ago(last.start, now)), "stale"),
                                Freshness::Historical => ("History".to_string(), "hist"),
                            };
                            let (lo, hi) = v.iter().fold((f64::MAX, f64::MIN), |(a, b), m| (a.min(m.value), b.max(m.value)));
                            view! {
                                <span class="tile-value tabular">{format!("{}", last.value.round())}<small>"\u{a0}bpm"</small></span>
                                <span class="tile-sub tabular">{format!("Measured at {}", time::format_ts(last.start, &tz, "%H:%M"))}</span>
                                <span class=format!("pill {cls}")>{pill}</span>
                                <span class="tile-sub tabular">{format!("Range {}\u{2060}–\u{2060}{}\u{a0}bpm", lo.round(), hi.round())}</span>
                            }.into_any()
                        }
                    },
                }
            }}
        </button>
    }
}

#[component]
fn SleepTile() -> impl IntoView {
    let app = use_app();
    view! {
        <button class="tile tone-sleep" on:click=move |_| app.go(Route::Metric(Metric::Sleep, Range::Day))>
            <span class="tile-label"><Icon name="moon" size=18 />"Sleep"</span>
            {move || {
                let day = app.day.get();
                let tz = app.tz();
                let s = app.settings.get();
                let latest = app.summaries.with(|i| i.latest(Metric::Sleep, day).cloned());
                match latest {
                    None => view! { <span class="tile-value muted">"—"</span><span class="tile-sub">"No sleep recorded"</span> }.into_any(),
                    Some(sm) => {
                        let v = sm.value.unwrap_or(0.0);
                        let sday = time::parse_day(&sm.day).unwrap_or(day);
                        // "Last night" only for today; past days name the night explicitly.
                        let when = if sday == day && day == app.today() {
                            "Last night".to_string()
                        } else if sday == day {
                            format!("Night ending {}", day_short(sday))
                        } else {
                            format!("Most recent: night ending {}", day_short(sday))
                        };
                        let (bed, wake, stages) = match &sm.extra {
                            Some(SummaryExtra::Sleep { bed, wake, stage_minutes, stages_measured, .. }) => (Some(*bed), Some(*wake), stages_measured.then(|| stage_minutes.clone())),
                            _ => (None, None, None),
                        };
                        let goal_pct = v / s.sleep_goal_min.max(1) as f64 * 100.0;
                        view! {
                            <span class="tile-value tabular">{time::fmt_minutes(v)}</span>
                            <span class="tile-sub">{when}</span>
                            {bed.zip(wake).map(|(b, w)| view! { <span class="tile-sub tabular">{format!("{} → {}", time::format_ts(b, &tz, "%H:%M"), time::format_ts(w, &tz, "%H:%M"))}</span> })}
                            {stages.map(|st| {
                                let total: f64 = st.values().sum::<f64>().max(1.0);
                                view! {
                                    <span class="stage-bar" aria-hidden="true">
                                        {["deep", "light", "rem", "awake"].into_iter().map(|k| {
                                            let p = st.get(k).copied().unwrap_or(0.0) / total * 100.0;
                                            view! { <span class=format!("stage-seg stage-{k}") style=format!("width:{p:.1}%")></span> }
                                        }).collect_view()}
                                    </span>
                                }
                            })}
                            <span class="tile-sub tabular">{format!("{goal_pct:.0}% of your {} goal", time::fmt_minutes(s.sleep_goal_min as f64))}</span>
                        }.into_any()
                    }
                }
            }}
        </button>
    }
}

#[component]
fn PairTiles() -> impl IntoView {
    let app = use_app();
    let tile = move |m: Metric, tone: &'static str| {
        view! {
            <button class=format!("tile small tone-{tone}") on:click=move |_| app.go(Route::Metric(m, Range::Day))>
                <span class="tile-label"><Icon name=metric_icon(m) size=18 />{catalog::info(m).title}</span>
                {move || {
                    let d = app.day.get();
                    let s = app.settings.get();
                    match app.summaries.with(|i| i.get(m, d).cloned()) {
                        Some(sm) if sm.value.is_some() => {
                            let partial = d == app.today();
                            let tz = app.tz();
                            // Say plainly when the value covers only part of the day.
                            let since = match &sm.extra {
                                Some(SummaryExtra::Additive { parts, .. }) => parts.iter().map(|p| p.from).min(),
                                _ => None,
                            }
                            .filter(|t| *t - time::day_start(d, &tz) > HOUR_MS);
                            let low = sm.coverage.map(|c| c < 0.9).unwrap_or(false);
                            let live = if sm.provisional { " · live estimate" } else { "" };
                            let sub = match (partial, since, low, m) {
                                (true, Some(t), _, _) => format!("Only since {}{live}", time::format_ts(t, &tz, "%H:%M")),
                                (true, None, _, _) => format!("So far today{live}"),
                                (false, _, true, _) => format!("{:.0}% of day recorded", sm.coverage.unwrap_or(0.0) * 100.0),
                                (false, _, false, Metric::Calories) => calorie_scope_label(&sm.extra).to_string(),
                                _ => String::new(),
                            };
                            view! {
                                <span class="tile-value tabular">{if sm.provisional { "≈\u{a0}" } else { "" }}{format::value(m, sm.value.unwrap(), &s)}</span>
                                {(!sub.is_empty()).then(|| view! { <span class="tile-sub">{sub}</span> })}
                            }.into_any()
                        }
                        _ => view! { <span class="tile-value muted">"—"</span><span class="tile-sub">"No data"</span> }.into_any(),
                    }
                }}
            </button>
        }
    };
    view! {
        {tile(Metric::Distance, "activity")}
        {tile(Metric::Calories, "activity")}
    }
}

fn calorie_scope_label(extra: &Option<SummaryExtra>) -> &'static str {
    match extra {
        Some(SummaryExtra::Additive { calorie_scope: Some(airspace_core::CalorieScope::Total), .. }) => "Total, incl. resting",
        Some(SummaryExtra::Additive { calorie_scope: Some(airspace_core::CalorieScope::Active), .. }) => "Active only",
        _ => "",
    }
}

#[component]
fn Trends() -> impl IntoView {
    let app = use_app();
    let rows = [(Metric::Steps, "activity"), (Metric::Sleep, "sleep"), (Metric::RestingHeartRate, "heart")];
    view! {
        <Section title="Trends">
            <ul class="trend-list">
                {rows.into_iter().map(|(m, tone)| view! {
                    <li>
                        <button class="trend-row" on:click=move |_| app.go(Route::Metric(m, Range::Week))>
                            {move || {
                                let day = app.day.get();
                                let s = app.settings.get();
                                // Today's steps are still growing: compare complete days only.
                                let end = if m.is_additive() && day >= app.today() { day - Duration::days(1) } else { day };
                                let (series, cmp) = app.summaries.with(|i| (
                                    i.series(m, day - Duration::days(13), day).into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
                                    compare_periods(i, m, end - Duration::days(6), end),
                                ));
                                let txt = match (cmp.current, cmp.reliable()) {
                                    (Some(c), true) => format!("{} avg · {} vs prior 7 days", format::value(m, c, &s), format::delta(m, cmp.delta().unwrap_or(0.0), &s)),
                                    (Some(c), false) => format!("{} avg · not enough data to compare", format::value(m, c, &s)),
                                    (None, _) => "No data in the last 7 days".to_string(),
                                };
                                view! {
                                    <span class="trend-name">{catalog::info(m).title}</span>
                                    <span class="trend-text tabular">{txt}</span>
                                    <span class="trend-cov">{format!("{} of 7 {}days recorded{}", cmp.current_days, if end < day { "complete " } else { "" }, if end < day { " (today excluded)" } else { "" })}</span>
                                    <Sparkline values=series tone=tone />
                                }
                            }}
                        </button>
                    </li>
                }).collect_view()}
            </ul>
        </Section>
    }
}

#[component]
fn Favorites() -> impl IntoView {
    let app = use_app();
    let editing = RwSignal::new(false);
    view! {
        <section class="section">
            <div class="section-head">
                <h2>"Favorites"</h2>
                <button class="link" aria-pressed=move || if editing.get() { "true" } else { "false" } on:click=move |_| editing.update(|e| *e = !*e)>
                    {move || if editing.get() { "Done" } else { "Edit" }}
                </button>
            </div>
            {move || if editing.get() {
                view! {
                    <ul class="choose-list">
                        {Metric::ALL.into_iter().map(|m| {
                            let on = move || app.settings.with(|s| s.favorites.contains(&m));
                            view! {
                                <li><label class="check-row">
                                    <input type="checkbox" prop:checked=on on:change=move |_| app.update_settings(|s| {
                                        if s.favorites.contains(&m) { s.favorites.retain(|x| *x != m) } else { s.favorites.push(m) }
                                    }) />
                                    <Icon name=metric_icon(m) size=18 />
                                    <span>{catalog::info(m).title}</span>
                                </label></li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            } else {
                view! { <FavoriteTiles /> }.into_any()
            }}
        </section>
    }
}

#[component]
fn FavoriteTiles() -> impl IntoView {
    let app = use_app();
    move || {
        let favs = app.settings.with(|s| s.favorites.clone());
        if favs.is_empty() {
            return view! { <p class="muted">"Choose metrics to pin here with Edit."</p> }.into_any();
        }
        view! {
            <div class="fav-grid">
                {favs.into_iter().map(|m| {
                    let info = catalog::info(m);
                    view! {
                        <button class=format!("tile small tone-{}", info.tone.css()) on:click=move |_| app.go(Route::Metric(m, Range::Week))>
                            <span class="tile-label"><Icon name=metric_icon(m) size=18 />{info.short}</span>
                            {move || {
                                let day = app.day.get();
                                let s = app.settings.get();
                                match app.summaries.with(|i| i.latest(m, day).cloned()) {
                                    Some(sm) => {
                                        let d = time::parse_day(&sm.day).unwrap_or(day);
                                        view! {
                                            <span class="tile-value tabular">{format::value(m, sm.value.unwrap_or(0.0), &s)}</span>
                                            <span class="tile-sub">{if d == app.today() { "Today".to_string() } else if d == day { day_short(d) } else { format!("Latest: {}", day_short(d)) }}</span>
                                        }.into_any()
                                    }
                                    None => view! { <span class="tile-value muted">"—"</span><span class="tile-sub">"No data"</span> }.into_any(),
                                }
                            }}
                        </button>
                    }
                }).collect_view()}
            </div>
        }.into_any()
    }
}

/// Last 7 days of steps (wide screens only). Days without data are flat.
#[component]
fn MiniWeek() -> impl IntoView {
    let app = use_app();
    move || {
        let day = app.day.get();
        let goal = app.settings.with(|s| s.step_goal as f64);
        let vals: Vec<(chrono::NaiveDate, Option<f64>)> = app.summaries.with(|i| i.series(Metric::Steps, day - Duration::days(6), day));
        let max = vals.iter().filter_map(|(_, v)| *v).fold(goal, f64::max).max(1.0);
        view! {
            <div class="mini-bars" aria-hidden="true">
                <span class="mini-caption">"Last 7 days"</span>
                {vals.into_iter().map(|(d, v)| {
                    let h = v.map(|v| (v / max * 72.0).max(2.0)).unwrap_or(2.0);
                    view! {
                        <span class="mini-bar" class:none=v.is_none() class:today={d == day}>
                            <span style=format!("height:{h:.0}px")></span>
                            <small>{d.format("%a").to_string().chars().next().unwrap_or(' ').to_string()}</small>
                        </span>
                    }
                }).collect_view()}
            </div>
        }
    }
}
