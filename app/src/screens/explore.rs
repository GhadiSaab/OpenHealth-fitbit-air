//! Explore: all metric groups, and a detail screen per metric.

use crate::state::{App, Range, Route, Sheet, async_signal, load_range};
use crate::ui::charts::{BarChart, DataTable, IBucket, IntradayChart, LineChart, Pt, STAGE_ROWS, SleepTimeline, Sparkline, StageSeg};
use crate::ui::format::{self, day_short};
use crate::ui::icons::{Icon, metric_icon};
use crate::ui::widgets::{DemoBanner, EmptyState, Header, Section, Segmented, chart_width, use_app};
use airspace_core::aggregate::{SummaryContext, SummaryExtra, additive_increments};
use airspace_core::catalog::{self, DailyChart, Group};
use airspace_core::insights::{circular_mean_minutes, fmt_clock, wrap_diff};
use airspace_core::series::{self, Point, std_dev};
use airspace_core::settings::{display_scale, display_unit};
use airspace_core::time::{self, HOUR_MS, MINUTE_MS};
use airspace_core::{Measurement, Metric, Origin, SleepStage};
use chrono::{Datelike, Duration, NaiveDate};
use leptos::prelude::*;
use std::collections::BTreeMap;
use std::sync::Arc;

#[component]
pub fn Explore() -> impl IntoView {
    let app = use_app();
    view! {
        <Header title="Explore" />
        <DemoBanner />
        <p class="lede">"Every metric Airspace can store. Tap one for its history, statistics and where the data came from."</p>
        <ul class="group-list">
            {Group::ALL.into_iter().map(|g| view! {
                <li class="group-card">
                    <h2 class="group-title">{g.title()}</h2>
                    {g.metrics().iter().map(|m| {
                        let m = *m;
                        let info = catalog::info(m);
                        view! {
                            <button class=format!("metric-row tone-{}", info.tone.css()) on:click=move |_| {
                                app.go(Route::Metric(m, if info.has_intraday { Range::Day } else { Range::Week }))
                            }>
                                <span class="metric-icon"><Icon name=metric_icon(m) /></span>
                                <span class="metric-name">{info.title}</span>
                                {move || {
                                    let today = app.today();
                                    let s = app.settings.get();
                                    app.summaries.with(|i| {
                                        let latest = i.latest(m, today).cloned();
                                        let series: Vec<Option<f64>> = i.series(m, today - Duration::days(13), today).into_iter().map(|(_, v)| v).collect();
                                        match latest {
                                            Some(sm) => view! {
                                                <span class="metric-value tabular">{format::value(m, sm.value.unwrap_or(0.0), &s)}</span>
                                                <span class="metric-when">{time::parse_day(&sm.day).map(|d| if d == today { "Today".into() } else { day_short(d) }).unwrap_or_default()}</span>
                                                <Sparkline values=series tone=info.tone.css() width=72.0 height=28.0 />
                                            }.into_any(),
                                            None => view! { <span class="metric-value muted">"No data"</span> }.into_any(),
                                        }
                                    })
                                }}
                                <Icon name="right" size=18 />
                            </button>
                        }
                    }).collect_view()}
                </li>
            }).collect_view()}
        </ul>
    }
}

/// The period shown for a range and anchor day.
fn period(range: Range, anchor: NaiveDate) -> (NaiveDate, NaiveDate) {
    match range {
        Range::Day => (anchor, anchor),
        Range::Week => {
            let s = time::week_start(anchor);
            (s, s + Duration::days(6))
        }
        Range::Month => {
            let s = time::month_start(anchor);
            (s, time::add_months(s, 1) - Duration::days(1))
        }
        Range::Year => (NaiveDate::from_ymd_opt(anchor.year(), 1, 1).unwrap(), NaiveDate::from_ymd_opt(anchor.year(), 12, 31).unwrap()),
    }
}

fn shift(range: Range, anchor: NaiveDate, dir: i32) -> NaiveDate {
    match range {
        Range::Day => anchor + Duration::days(dir as i64),
        Range::Week => anchor + Duration::days(7 * dir as i64),
        Range::Month => time::add_months(anchor, dir),
        Range::Year => time::add_months(anchor, 12 * dir),
    }
}

fn period_label(range: Range, from: NaiveDate, to: NaiveDate) -> String {
    match range {
        Range::Day => from.format("%A, %-d %B %Y").to_string(),
        Range::Week => {
            if from.month() == to.month() {
                format!("{}–{} {}", from.day(), to.day(), to.format("%B %Y"))
            } else {
                format!("{} – {}", from.format("%-d %b"), to.format("%-d %b %Y"))
            }
        }
        Range::Month => from.format("%B %Y").to_string(),
        Range::Year => from.format("%Y").to_string(),
    }
}

#[component]
pub fn MetricDetail(metric: Metric, range: Range) -> impl IntoView {
    let app = use_app();
    let info = catalog::info(metric);
    let mut ranges = vec![];
    if info.has_intraday {
        ranges.push((Range::Day, "Day"));
    }
    ranges.extend([(Range::Week, "Week"), (Range::Month, "Month"), (Range::Year, "Year")]);
    let range = if !info.has_intraday && range == Range::Day { Range::Week } else { range };
    let range_sig = Signal::derive(move || range);
    let on_range = Callback::new(move |r: Range| app.route.set(Route::Metric(metric, r)));
    let anchor = app.day;
    let bounds = move || period(range, anchor.get());
    let at_end = move || bounds().1 >= app.today();
    let has_any = move || app.summaries.with(|i| i.has_metric(metric));
    let show_table = RwSignal::new(false);

    view! {
        <Header title=info.title back=true />
        <DemoBanner />
        {move || if !has_any() {
            view! { <MetricEmpty metric=metric /> }.into_any()
        } else {
            view! {
                <div class="detail-controls">
                    <Segmented options=ranges.clone() value=range_sig on_change=on_range label="Time range" />
                    <div class="period-nav" role="group" aria-label="Period">
                        <button class="icon-btn" aria-label="Previous period" on:click=move |_| anchor.update(|a| *a = shift(range, *a, -1))><Icon name="left" /></button>
                        <span class="period-label tabular" aria-live="polite">{move || { let (a, b) = bounds(); period_label(range, a, b) }}</span>
                        <button class="icon-btn" aria-label="Next period" disabled=at_end on:click=move |_| { if !at_end() { anchor.update(|a| *a = shift(range, *a, 1).min(app.today())) } }><Icon name="right" /></button>
                    </div>
                </div>
                <div class="detail-card">
                    {move || {
                        let (from, to) = bounds();
                        if range == Range::Day {
                            view! { <DayView metric=metric day=from /> }.into_any()
                        } else {
                            view! { <PeriodView metric=metric range=range from=from to=to show_table=show_table /> }.into_any()
                        }
                    }}
                </div>
                <PeriodStats metric=metric range=range />
                <MetricExtras metric=metric range=range />
            }.into_any()
        }}
        <Section title="About this measurement">
            <p>{info.explanation}</p>
            <p class="muted">{move || format!("Unit: {}.", display_unit(metric, &app.settings.get()))}" "{info.caution}</p>
            {matches!(metric, Metric::Sleep | Metric::Spo2 | Metric::Hrv | Metric::RespiratoryRate | Metric::SkinTemperature | Metric::RestingHeartRate | Metric::HeartRateZones).then(|| view! {
                <p class="muted">"This metric comes only from imported history. Airspace does not infer it from the live Bluetooth stream."</p>
            })}
        </Section>
        <SourcesSection metric=metric range=range />
    }
}

#[component]
fn MetricEmpty(metric: Metric) -> impl IntoView {
    let app = use_app();
    let info = catalog::info(metric);
    let live = airspace_core::bluetooth::live_capabilities().into_iter().find(|(m, _)| *m == metric).map(|(_, s)| s);
    let how = match live {
        Some(airspace_core::bluetooth::Support::Unsupported) => "It can only come from imported history — the live Bluetooth stream doesn't provide it.",
        _ => "Import history to see it here. Live Bluetooth recording will be able to add it in a future version.",
    };
    view! {
        <EmptyState icon=metric_icon(metric) title=format!("No {} data yet", info.short.to_lowercase()) body=format!("{} {how}", info.explanation.split(". ").next().unwrap_or("")) >
            <button class="btn" on:click=move |_| app.go(Route::Import)>"Import history"</button>
        </EmptyState>
    }
}

fn pts_for_days(app: &App, metric: Metric, from: NaiveDate, to: NaiveDate) -> Vec<Pt> {
    let s = app.settings.get();
    let today = app.today();
    app.summaries.with(|i| {
        time::days_inclusive(from, to)
            .map(|d| {
                let sm = i.get(metric, d);
                let v = sm.and_then(|x| x.value);
                let (lo, hi) = match catalog::info(metric).daily_chart {
                    DailyChart::RangeLine => (sm.and_then(|x| x.min).map(|v| display_scale(metric, v, &s)), sm.and_then(|x| x.max).map(|v| display_scale(metric, v, &s))),
                    _ => (None, None),
                };
                let mut note = vec![];
                if let Some(c) = sm.and_then(|x| x.coverage).filter(|c| *c < 0.9 && metric != Metric::Steps) {
                    note.push(format!("{:.0}% of day recorded", c * 100.0));
                }
                if sm.map(|x| x.provisional).unwrap_or(false) {
                    note.push("includes provisional values".into());
                }
                let text = match (metric, v, sm) {
                    (Metric::HeartRate, Some(v), Some(x)) => format!("avg {} · {}–{}", format::value(metric, v, &s), x.min.unwrap_or(v).round(), x.max.unwrap_or(v).round()),
                    (Metric::Spo2, Some(v), Some(x)) => format!("{} (range {:.0}–{:.0}%)", format::value(metric, v, &s), x.min.unwrap_or(v), x.max.unwrap_or(v)),
                    (Metric::Sleep, Some(v), _) => time::fmt_minutes(v),
                    (_, Some(v), _) => format::value(metric, v, &s),
                    _ => String::new(),
                };
                Pt {
                    future: d > today,
                    label: d.format("%a %-d %b %Y").to_string(),
                    tick: if (to - from).num_days() > 10 { d.day().to_string() } else { d.format("%a").to_string() },
                    v: v.map(|v| display_scale(metric, v, &s)),
                    lo,
                    hi,
                    text,
                    note: if note.is_empty() { None } else { Some(note.join(" · ")) },
                }
            })
            .collect()
    })
}

/// Weekly means for the year view.
fn pts_for_year(app: &App, metric: Metric, from: NaiveDate, to: NaiveDate) -> Vec<Pt> {
    let s = app.settings.get();
    let mut out = vec![];
    let mut w = time::week_start(from);
    app.summaries.with(|i| {
        while w <= to {
            let we = w + Duration::days(6);
            let vals: Vec<f64> = i.series(metric, w, we).into_iter().filter_map(|(_, v)| v).collect();
            let mean = if vals.is_empty() { None } else { Some(vals.iter().sum::<f64>() / vals.len() as f64) };
            out.push(Pt {
                future: w > app.today(),
                label: format!("Week of {}", w.format("%-d %b %Y")),
                tick: if w.day() <= 7 { w.format("%b").to_string() } else { String::new() },
                v: mean.map(|v| display_scale(metric, v, &s)),
                lo: None,
                hi: None,
                text: mean.map(|v| format!("{} {}", if metric == Metric::Sleep { time::fmt_minutes(v) } else { format::value(metric, v, &s) }, if metric.is_additive() || metric == Metric::Sleep { "per day (avg)" } else { "(avg)" })).unwrap_or_default(),
                note: Some(format!("{} of 7 days recorded", vals.len())),
            });
            w += Duration::days(7);
        }
    });
    out
}

#[component]
fn PeriodView(metric: Metric, range: Range, from: NaiveDate, to: NaiveDate, show_table: RwSignal<bool>) -> impl IntoView {
    let app = use_app();
    let info = catalog::info(metric);
    let tone = info.tone.css();
    let width = move || chart_width(app.vw.get());
    let goal = move || match metric {
        Metric::Steps => Some(app.settings.with(|s| s.step_goal as f64)),
        Metric::Sleep => Some(app.settings.with(|s| s.sleep_goal_min as f64 / 60.0)),
        _ => None,
    };
    let title = format!("{} for {}", info.title, period_label(range, from, to));
    let pts = move || if range == Range::Year { pts_for_year(&app, metric, from, to) } else { pts_for_days(&app, metric, from, to) };
    view! {
        {move || {
            let p = pts();
            let t = title.clone();
            match info.daily_chart {
                DailyChart::Bars => view! { <BarChart pts=p tone=tone title=t goal=goal() width=width() /> }.into_any(),
                DailyChart::Line | DailyChart::RangeLine => view! { <LineChart pts=p tone=tone title=t width=width() /> }.into_any(),
                DailyChart::Deviation => view! { <LineChart pts=p tone=tone title=t width=width() deviation=true /> }.into_any(),
            }
        }}
        <div class="chart-foot">
            <span class="muted">{move || format!("Values in {}. Dots on the baseline mark days with no data.", display_unit(metric, &app.settings.get()))}</span>
            <button class="link" aria-expanded=move || if show_table.get() { "true" } else { "false" } on:click=move |_| show_table.update(|v| *v = !*v)>
                <Icon name="table" size=18 />{move || if show_table.get() { "Hide table" } else { "Show table" }}
            </button>
        </div>
        {move || show_table.get().then(|| {
            let p = pts();
            let rows = p.iter().filter(|x| !x.future).map(|x| vec![x.label.rsplit_once(' ').map(|(a, _)| a.to_string()).unwrap_or_else(|| x.label.clone()), if x.v.is_some() { x.text.clone() } else { "No data".into() }, x.note.clone().unwrap_or_default()]).collect();
            view! {
                <DataTable caption=format!("{} — table", info.title) headers=vec!["Date", "Value", "Notes"] rows=rows />
                {(range != Range::Year).then(|| view! {
                    <p class="muted">"For the source and method behind a single day, open its details:"</p>
                    <div class="day-chips">
                        {time::days_inclusive(from, to).filter(|d| app.summaries.with_untracked(|i| i.get(metric, *d).is_some())).map(|d| view! {
                            <button class="chip" on:click=move |_| app.sheet.set(Some(Sheet::Provenance(metric, d)))>{d.format("%-d %b").to_string()}</button>
                        }).collect_view()}
                    </div>
                })}
            }
        })}
    }
}

#[component]
fn DayView(metric: Metric, day: NaiveDate) -> impl IntoView {
    let app = use_app();
    let samples = async_signal(
        move || (app.db.get(), app.data_rev.get()),
        move |_| {
            let (a, b) = if metric == Metric::Sleep { (day, day) } else { (day, day) };
            load_range(app, metric, a, b)
        },
    );
    let width = move || chart_width(app.vw.get());
    let show_table = RwSignal::new(false);
    view! {
        {move || match samples.get() {
            None => view! { <div class="chart-skeleton" aria-busy="true"></div> }.into_any(),
            Some(ms) if ms.is_empty() => view! {
                <div class="chart-empty"><p><strong>{format!("No {} recorded on {}.", catalog::info(metric).short.to_lowercase(), day_short(day))}</strong></p>
                <p class="muted">"The tracker may not have been worn, or this day hasn't been imported."</p></div>
            }.into_any(),
            Some(ms) => {
                let ms = Arc::new(ms);
                match metric {
                    Metric::HeartRate => hr_day(&app, &ms, day, width(), show_table).into_any(),
                    Metric::Sleep => sleep_day(&app, &ms, width(), show_table).into_any(),
                    _ => additive_day(&app, metric, &ms, day, width(), show_table).into_any(),
                }
            }
        }}
    }
}

fn hour_labels(app: &App, day: NaiveDate) -> Vec<(i64, String)> {
    let tz = app.tz_untracked();
    let (s, _) = time::day_bounds(day, &tz);
    (0..=4).map(|i| (s + i * 6 * HOUR_MS, format!("{:02}:00", (i * 6) % 24))).collect()
}

fn table_toggle(show: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="chart-foot">
            <span></span>
            <button class="link" aria-expanded=move || if show.get() { "true" } else { "false" } on:click=move |_| show.update(|v| *v = !*v)>
                <Icon name="table" size=18 />{move || if show.get() { "Hide table" } else { "Show table" }}
            </button>
        </div>
    }
}

fn hr_day(app: &App, ms: &Arc<Vec<Measurement>>, day: NaiveDate, width: f64, show: RwSignal<bool>) -> impl IntoView {
    let tz = app.tz_untracked();
    let (from, to) = time::day_bounds(day, &tz);
    let mut pts: Vec<Point> = ms.iter().map(|m| Point { t: m.start, v: m.value }).collect();
    pts.sort_by_key(|p| p.t);
    let threshold = series::gap_threshold(&pts, 10 * MINUTE_MS);
    let end = to.min(app.now.get_untracked());
    let gaps: Vec<(i64, i64)> = series::gaps(&pts, from, end, threshold).into_iter().map(|g| (g.start, g.end)).collect();
    let width_ms = series::choose_width(from, to, 144, series::median_interval(&pts));
    let tz2 = tz;
    let buckets: Vec<IBucket> = series::bucketize(&pts, from, to, width_ms)
        .into_iter()
        .map(|b| IBucket {
            t0: b.t0,
            t1: b.t1,
            avg: b.avg,
            min: b.min,
            max: b.max,
            label: format!("{}–{}", time::format_ts(b.t0, &tz2, "%H:%M"), time::format_ts(b.t1, &tz2, "%H:%M")),
            text: if b.n > 1 { format!("{:.0} bpm avg · {:.0}–{:.0}", b.avg, b.min, b.max) } else { format!("{:.0} bpm", b.avg) },
        })
        .collect();
    let gap_total: i64 = gaps.iter().map(|(a, b)| b - a).sum();
    let live = ms.iter().filter(|m| m.prov.origin == Origin::BluetoothLive).count();
    let rows: Vec<Vec<String>> = buckets.iter().map(|b| vec![b.label.clone(), b.text.clone()]).collect();
    let gaps_txt: Vec<String> = gaps.iter().map(|(a, b)| format!("{}–{}", time::format_ts(*a, &tz, "%H:%M"), time::format_ts(*b, &tz, "%H:%M"))).collect();
    view! {
        <IntradayChart buckets=buckets gaps=gaps.clone() from=from to=to tone="heart" title=format!("Heart rate on {}", day_short(day)) hour_labels=hour_labels(app, day) width=width max_gap=threshold />
        <p class="chart-note muted">
            {format!("{} samples, shown as {}-minute averages with the min–max range. ", ms.len(), width_ms / MINUTE_MS)}
            {if gaps.is_empty() { "No recording gaps.".to_string() } else { format!("{} without recording ({}).", time::fmt_minutes((gap_total / MINUTE_MS) as f64), gaps_txt.join(", ")) }}
            {(live > 0).then(|| format!(" {live} samples came from Bluetooth recording; the rest from imported history."))}
        </p>
        {table_toggle(show)}
        {move || show.get().then(|| view! { <DataTable caption="Heart rate by time" headers=vec!["Time", "Heart rate"] rows=rows.clone() /> })}
    }
}

fn additive_day(app: &App, metric: Metric, ms: &Arc<Vec<Measurement>>, day: NaiveDate, width: f64, show: RwSignal<bool>) -> impl IntoView {
    let tz = app.tz_untracked();
    let s = app.settings.get_untracked();
    let (from, to) = time::day_bounds(day, &tz);
    let refs: Vec<&Measurement> = ms.iter().collect();
    let ctx = SummaryContext { tz, now: app.now.get_untracked(), demo: app.is_demo() };
    let incs = additive_increments(metric, day, &refs, &ctx);
    // Sum increments into hourly buckets (an increment spanning hours is split proportionally).
    let mut hours: BTreeMap<i64, f64> = BTreeMap::new();
    let mut daily_only = false;
    for inc in &incs {
        if inc.to - inc.from >= 20 * HOUR_MS {
            daily_only = true;
            continue;
        }
        let mut t = inc.from;
        while t < inc.to {
            let h0 = from + ((t - from) / HOUR_MS) * HOUR_MS;
            let seg_end = (h0 + HOUR_MS).min(inc.to);
            let share = (seg_end - t) as f64 / (inc.to - inc.from).max(1) as f64;
            *hours.entry(h0).or_default() += inc.value * share;
            t = seg_end;
        }
    }
    let buckets: Vec<IBucket> = hours
        .iter()
        .map(|(h, v)| {
            let dv = display_scale(metric, *v, &s);
            IBucket {
                t0: *h,
                t1: h + HOUR_MS,
                avg: dv,
                min: dv,
                max: dv,
                label: format!("{}–{}", time::format_ts(*h, &tz, "%H:%M"), time::format_ts(h + HOUR_MS, &tz, "%H:%M")),
                text: format::value(metric, *v, &s),
            }
        })
        .collect();
    let counters = ms.iter().any(|m| m.kind == airspace_core::SampleKind::CumulativeCounter);
    let rows: Vec<Vec<String>> = buckets.iter().map(|b| vec![b.label.clone(), b.text.clone()]).collect();
    let title = format!("{} per hour on {}", catalog::info(metric).title, day_short(day));
    view! {
        {if buckets.is_empty() {
            view! { <div class="chart-empty"><p>{if daily_only { "Only a daily total is available for this day; there is no hourly breakdown." } else { "No hourly data." }}</p></div> }.into_any()
        } else {
            view! { <IntradayChart buckets=buckets from=from to=to gaps=vec![] tone="activity" title=title hour_labels=hour_labels(app, day) bars=true width=width /> }.into_any()
        }}
        <p class="chart-note muted">
            {"Hourly totals use the same source rules as the daily total, so they add up to it. "}
            {counters.then_some("Live values come from the device's running counter; Airspace counts the change between readings, never the readings themselves.")}
        </p>
        {table_toggle(show)}
        {move || show.get().then(|| view! { <DataTable caption="Hourly totals" headers=vec!["Hour", "Total"] rows=rows.clone() /> })}
    }
}

fn sleep_day(app: &App, ms: &Arc<Vec<Measurement>>, width: f64, show: RwSignal<bool>) -> impl IntoView {
    let tz = app.tz_untracked();
    let main = ms.iter().filter(|m| m.sleep().map(|d| d.is_main).unwrap_or(true)).max_by_key(|m| m.end_or_start() - m.start).cloned();
    let Some(main) = main else { return view! { <p>"No main sleep session."</p> }.into_any() };
    let d = main.sleep().cloned();
    let from = main.start;
    let to = main.end_or_start();
    let segs: Vec<StageSeg> = d
        .as_ref()
        .map(|d| {
            d.stages
                .iter()
                .map(|s| StageSeg {
                    stage: match s.stage {
                        SleepStage::Awake => "awake",
                        SleepStage::Rem => "rem",
                        SleepStage::Deep => "deep",
                        _ => "light",
                    },
                    start: s.start,
                    end: s.end,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut labels = vec![];
    let mut t = from - from.rem_euclid(HOUR_MS) + HOUR_MS;
    let step = if to - from > 6 * HOUR_MS { 2 * HOUR_MS } else { HOUR_MS };
    while t < to {
        labels.push((t, time::format_ts(t, &tz, "%H:%M")));
        t += step;
    }
    let tz2 = tz;
    let describe: Arc<dyn Fn(&StageSeg) -> String + Send + Sync> = Arc::new(move |s: &StageSeg| {
        format!("{} · {}–{}", time::fmt_minutes(((s.end - s.start) / MINUTE_MS) as f64), time::format_ts(s.start, &tz2, "%H:%M"), time::format_ts(s.end, &tz2, "%H:%M"))
    });
    let staged = !segs.is_empty();
    let rows: Vec<Vec<String>> = segs
        .iter()
        .map(|s| vec![format!("{}–{}", time::format_ts(s.start, &tz, "%H:%M"), time::format_ts(s.end, &tz, "%H:%M")), STAGE_ROWS.iter().find(|r| r.0 == s.stage).map(|r| r.1).unwrap_or("").into(), time::fmt_minutes(((s.end - s.start) / MINUTE_MS) as f64)])
        .collect();
    let mut totals: BTreeMap<&'static str, f64> = BTreeMap::new();
    for s in &segs {
        *totals.entry(s.stage).or_default() += ((s.end - s.start) / MINUTE_MS) as f64;
    }
    view! {
        <div class="sleep-head">
            <div><span class="big tabular">{time::fmt_minutes(main.value)}</span><span class="muted">" asleep"</span></div>
            <div class="muted tabular">{format!("{} → {} · {} in bed", time::format_ts(from, &tz, "%H:%M"), time::format_ts(to, &tz, "%H:%M"), time::fmt_minutes(((to - from) / MINUTE_MS) as f64))}</div>
        </div>
        {if staged {
            view! {
                <SleepTimeline segs=segs from=from to=to labels=labels title="Sleep stages" width=width describe=describe />
                <ul class="legend-inline" aria-label="Stage totals (calculated by Airspace from the stage intervals)">
                    {STAGE_ROWS.iter().map(|(k, l)| view! {
                        <li><span class=format!("swatch stage-{k}") aria-hidden="true"></span>{format!("{l} {}", time::fmt_minutes(totals.get(k).copied().unwrap_or(0.0)))}</li>
                    }).collect_view()}
                </ul>
                <p class="chart-note muted">"Stage intervals as reported by the source. Totals are calculated by Airspace."</p>
                {table_toggle(show)}
                {move || show.get().then(|| view! { <DataTable caption="Sleep stages" headers=vec!["Time", "Stage", "Duration"] rows=rows.clone() /> })}
            }.into_any()
        } else {
            view! { <div class="chart-empty"><p>"This night has a start and end time but no stage data."</p></div> }.into_any()
        }}
    }
    .into_any()
}

#[component]
fn PeriodStats(metric: Metric, range: Range) -> impl IntoView {
    let app = use_app();
    move || {
        let (from, to) = period(range, app.day.get());
        let len = (to - from).num_days() + 1;
        let (pfrom, pto) = (from - Duration::days(len), from - Duration::days(1));
        let s = app.settings.get();
        let (cur, prev) = app.summaries.with(|i| {
            let c: Vec<f64> = i.series(metric, from, to).into_iter().filter_map(|(_, v)| v).collect();
            let p: Vec<f64> = i.series(metric, pfrom, pto).into_iter().filter_map(|(_, v)| v).collect();
            (c, p)
        });
        let today = app.today();
        let elapsed = (to.min(today) - from).num_days() + 1;
        let st = series::stats(cur.iter().copied());
        let pst = series::stats(prev.iter().copied());
        let s2 = s.clone();
        let fmt = move |v: f64| if metric == Metric::Sleep { time::fmt_minutes(v) } else { format::value(metric, v, &s2) };
        let avg_label = if metric.is_additive() || metric == Metric::Sleep { "Daily average" } else { "Average" };
        let reliable = (st.n as i64) * 2 >= elapsed.max(1) && (pst.n as i64) * 2 >= len && st.n > 0;
        let label = match range { Range::Day => "previous day", Range::Week => "previous week", Range::Month => "previous month", Range::Year => "previous year" };
        let cmp = match (st.mean, pst.mean, reliable) {
            (Some(a), Some(b), true) => format!("{} vs {label} ({}).", format::delta(metric, a - b, &s), fmt(b)),
            (Some(_), _, _) => format!("Not enough data to compare with the {label}."),
            _ => String::new(),
        };
        if range == Range::Day {
            return view! {}.into_any();
        }
        view! {
            <Section title="Summary">
                {if st.n == 0 {
                    view! { <p class="muted">"No data in this period."</p> }.into_any()
                } else {
                    view! {
                        <dl class="stat-grid">
                            <div><dt>{avg_label}</dt><dd class="tabular">{st.mean.map(|v| fmt(v)).unwrap_or_default()}</dd></div>
                            <div><dt>"Lowest day"</dt><dd class="tabular">{st.min.map(|v| fmt(v)).unwrap_or_default()}</dd></div>
                            <div><dt>"Highest day"</dt><dd class="tabular">{st.max.map(|v| fmt(v)).unwrap_or_default()}</dd></div>
                            {metric.is_additive().then(|| view! { <div><dt>"Total"</dt><dd class="tabular">{fmt(st.sum)}</dd></div> })}
                            <div><dt>"Days with data"</dt><dd class="tabular">{format!("{} of {}", st.n, elapsed.max(0))}</dd></div>
                        </dl>
                        <p class="compare">{cmp}<span class="muted">{format!(" Coverage: {} of {} days this period, {} of {} before.", st.n, elapsed.max(0), pst.n, len)}</span></p>
                        {(metric == Metric::Steps).then(|| {
                            let goal = s.step_goal as f64;
                            let met = cur.iter().filter(|v| **v >= goal).count();
                            view! { <p class="muted">{format!("Goal reached on {met} of {} recorded days.", cur.len())}</p> }
                        })}
                    }.into_any()
                }}
            </Section>
        }.into_any()
    }
}

/// Metric-specific summaries: sleep timing and stages, zones, HRV method.
#[component]
fn MetricExtras(metric: Metric, range: Range) -> impl IntoView {
    let app = use_app();
    move || {
        let (from, to) = if range == Range::Day { (app.day.get() - Duration::days(6), app.day.get()) } else { period(range, app.day.get()) };
        let tz = app.tz();
        let s = app.settings.get();
        let extras: Vec<SummaryExtra> = app.summaries.with(|i| time::days_inclusive(from, to).filter_map(|d| i.get(metric, d).and_then(|x| x.extra.clone())).collect());
        match metric {
            Metric::Sleep => {
                let mut beds = vec![];
                let mut wakes = vec![];
                let mut stage: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for e in &extras {
                    if let SummaryExtra::Sleep { bed, wake, stage_minutes, stages_measured, .. } = e {
                        beds.push(time::minute_of_day(*bed, &tz) as f64);
                        wakes.push(time::minute_of_day(*wake, &tz) as f64);
                        if *stages_measured {
                            for (k, v) in stage_minutes {
                                stage.entry(k.clone()).or_default().push(*v);
                            }
                        }
                    }
                }
                if beds.len() < 2 {
                    return view! {}.into_any();
                }
                let cb = circular_mean_minutes(&beds).unwrap_or(0.0);
                let cw = circular_mean_minutes(&wakes).unwrap_or(0.0);
                let sdb = std_dev(&beds.iter().map(|m| wrap_diff(*m, cb)).collect::<Vec<_>>()).unwrap_or(0.0);
                let sdw = std_dev(&wakes.iter().map(|m| wrap_diff(*m, cw)).collect::<Vec<_>>()).unwrap_or(0.0);
                view! {
                    <Section title="Timing and stages">
                        <dl class="stat-grid">
                            <div><dt>"Typical sleep start"</dt><dd class="tabular">{fmt_clock(cb)}</dd></div>
                            <div><dt>"Typical wake"</dt><dd class="tabular">{fmt_clock(cw)}</dd></div>
                            <div><dt>"Start varies by"</dt><dd class="tabular">{format!("±{:.0} min", sdb)}</dd></div>
                            <div><dt>"Wake varies by"</dt><dd class="tabular">{format!("±{:.0} min", sdw)}</dd></div>
                        </dl>
                        {(!stage.is_empty()).then(|| view! {
                            <p class="muted">"Average minutes per stage on nights with stage data (calculated):"</p>
                            <ul class="legend-inline">
                                {STAGE_ROWS.iter().map(|(k, l)| {
                                    let v = stage.get(*k).map(|v| v.iter().sum::<f64>() / v.len() as f64).unwrap_or(0.0);
                                    view! { <li><span class=format!("swatch stage-{k}") aria-hidden="true"></span>{format!("{l} {}", time::fmt_minutes(v))}</li> }
                                }).collect_view()}
                            </ul>
                        })}
                        <p class="muted">{format!("Based on {} nights in this period. Variation is the standard deviation.", beds.len())}</p>
                    </Section>
                }.into_any()
            }
            Metric::HeartRateZones => {
                let mut mins: BTreeMap<String, f64> = BTreeMap::new();
                let mut bounds = BTreeMap::new();
                let mut calculated = false;
                for e in &extras {
                    if let SummaryExtra::Zones { minutes, bounds: b, origin } = e {
                        for (k, v) in minutes {
                            *mins.entry(k.clone()).or_default() += v;
                        }
                        bounds.extend(b.clone());
                        calculated |= *origin == airspace_core::ZoneOrigin::Calculated;
                    }
                }
                if mins.is_empty() {
                    return view! {}.into_any();
                }
                let rows = ["fat_burn", "cardio", "peak"]
                    .iter()
                    .map(|k| {
                        let (lo, hi) = bounds.get(*k).cloned().unwrap_or((None, None));
                        vec![
                            airspace_core::HrZone::parse(k).map(|z| z.label()).unwrap_or(k).to_string(),
                            format!("{:.0} min", mins.get(*k).copied().unwrap_or(0.0)),
                            match (lo, hi) { (Some(a), Some(b)) => format!("{a:.0}–{b:.0} bpm"), _ => "Not provided".into() },
                        ]
                    })
                    .collect();
                view! {
                    <Section title="Time in zones">
                        <DataTable caption="Minutes per zone in this period" headers=vec!["Zone", "Time", "Boundaries"] rows=rows />
                        <p class="muted">{if calculated { "Some zone boundaries were calculated by Airspace." } else { "Zone boundaries and minutes were imported from the source; Airspace did not calculate them." }}</p>
                    </Section>
                }.into_any()
            }
            Metric::Hrv => {
                let excluded: u32 = extras.iter().map(|e| if let SummaryExtra::Hrv { excluded_other_methods, .. } = e { *excluded_other_methods } else { 0 }).sum();
                let method = extras.iter().find_map(|e| if let SummaryExtra::Hrv { method, .. } = e { Some(method.label()) } else { None }).unwrap_or("unspecified");
                view! {
                    <Section title="Measurement method">
                        <p>{format!("Values shown are {method}, in milliseconds.")}</p>
                        {(excluded > 0).then(|| view! { <p class="muted">{format!("{excluded} value(s) in this period were measured with a different method and are kept separate, because HRV methods are not comparable.")}</p> })}
                    </Section>
                }.into_any()
            }
            Metric::RespiratoryRate => {
                let mut by: BTreeMap<String, Vec<f64>> = BTreeMap::new();
                for e in &extras {
                    if let SummaryExtra::Respiratory { by_stage } = e {
                        for (k, v) in by_stage {
                            by.entry(k.clone()).or_default().push(*v);
                        }
                    }
                }
                if by.is_empty() {
                    return view! {}.into_any();
                }
                view! {
                    <Section title="By sleep stage">
                        <dl class="stat-grid">
                            {by.into_iter().map(|(k, v)| view! {
                                <div><dt>{SleepStage::parse(&k).map(|s| match s { SleepStage::Rem => "REM", SleepStage::Deep => "Deep", SleepStage::Light => "Light", _ => "Other" }).unwrap_or("Other")}</dt>
                                <dd class="tabular">{format::value(Metric::RespiratoryRate, v.iter().sum::<f64>() / v.len() as f64, &s)}</dd></div>
                            }).collect_view()}
                        </dl>
                        <p class="muted">"Averages of stage-specific values reported by the source."</p>
                    </Section>
                }.into_any()
            }
            Metric::SkinTemperature => view! {
                <Section title="Reading this chart">
                    <p>"Zero is your personal baseline as calculated by the source. Points above it mean your skin was warmer than usual overnight; this is not a fever reading and not core body temperature."</p>
                </Section>
            }.into_any(),
            _ => view! {}.into_any(),
        }
    }
}

#[component]
fn SourcesSection(metric: Metric, range: Range) -> impl IntoView {
    let app = use_app();
    move || {
        let (from, to) = period(range, app.day.get());
        let tz = app.tz();
        let (origins, prov, latest, records, demo) = app.summaries.with(|i| {
            let mut o = std::collections::BTreeSet::new();
            let (mut p, mut l, mut n, mut d) = (false, None::<i64>, 0u32, false);
            for day in time::days_inclusive(from, to) {
                if let Some(s) = i.get(metric, day) {
                    o.extend(s.origins.iter().copied());
                    p |= s.provisional;
                    l = l.max(s.latest_ts);
                    n += s.count;
                    d |= s.demo;
                }
            }
            (o, p, l, n, d)
        });
        if records == 0 {
            return view! {}.into_any();
        }
        let src: Vec<&str> = origins.iter().map(|o| match o { Origin::HistoryImport => "imported history", Origin::BluetoothLive => "Bluetooth recording", Origin::Derived => "calculated" }).collect();
        view! {
            <Section title="Source and completeness">
                <dl class="facts">
                    <dt>"Source"</dt><dd>{format!("{}{}", src.join(" and "), if demo { " (simulated demo)" } else { "" })}</dd>
                    <dt>"Records"</dt><dd class="tabular">{records.to_string()}</dd>
                    {latest.map(|t| view! { <dt>"Latest measurement"</dt><dd class="tabular">{time::format_ts(t, &tz, "%a %-d %b %Y, %H:%M")}</dd> })}
                    {prov.then(|| view! { <dt>"Provisional"</dt><dd>"Some values use a live Bluetooth field mapping that hasn't been validated. They're marked with ≈ on Today."</dd> })}
                </dl>
                <button class="link" on:click=move |_| app.sheet.set(Some(Sheet::Provenance(metric, app.day.get_untracked())))>
                    {move || format!("Details for {}", day_short(app.day.get()))}
                </button>
            </Section>
        }.into_any()
    }
}
