//! Accessible SVG charts.
//!
//! Conventions (see docs/DESIGN.md): bars ≤ 24 px with a 4 px rounded data
//! end and square baseline; 2 px lines; markers r = 4 with a 2 px surface
//! ring; hairline solid gridlines; text in ink tokens, never series colors.
//! Missing data is drawn as an explicit "no data" mark, never as zero, and
//! lines are never drawn across gaps.
//!
//! Every chart is focusable: ←/→ move the selection, Home/End jump, and the
//! readout above the chart is an `aria-live` region. A table alternative is
//! provided by [`DataTable`].

use leptos::prelude::*;
use std::sync::Arc;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug, PartialEq)]
pub struct Pt {
    /// Category label for the readout (e.g. "Tue 3 Nov").
    pub label: String,
    /// Short axis label (e.g. "3").
    pub tick: String,
    pub v: Option<f64>,
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    /// Readout text for the value (already formatted with units).
    pub text: String,
    /// Small secondary note (coverage, provisional …).
    pub note: Option<String>,
    /// A day that hasn't happened yet: drawn as nothing, not as "no data".
    pub future: bool,
}

pub use airspace_core::series::nice_ticks;

pub fn fmt_tick(v: f64) -> String {
    let a = v.abs();
    if a >= 10_000.0 {
        format!("{}K", (v / 1000.0).round())
    } else if a >= 1000.0 {
        let s = format!("{}", v.round() as i64);
        let (h, t) = s.split_at(s.len() - 3);
        format!("{h},{t}")
    } else if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// Path for an optional trend overlay (e.g. a 7-day rolling average),
/// broken wherever the trend is undefined.
fn trend_path(trend: &[Option<f64>], x: impl Fn(usize) -> f64, y: impl Fn(f64) -> f64) -> String {
    let mut d = String::new();
    let mut prev = false;
    for (i, v) in trend.iter().enumerate() {
        match v {
            Some(v) => {
                d.push_str(&format!("{}{:.1},{:.1}", if prev { "L" } else { "M" }, x(i), y(*v)));
                prev = true;
            }
            None => prev = false,
        }
    }
    d
}

/// A spoken summary of a chart's data for screen readers.
pub fn summarize_pts(title: &str, pts: &[Pt]) -> String {
    let with: Vec<&Pt> = pts.iter().filter(|p| p.v.is_some()).collect();
    let past = pts.iter().filter(|p| !p.future).count();
    if with.is_empty() {
        return format!("{title}. No data recorded in this period.");
    }
    let lo = with.iter().min_by(|a, b| a.v.partial_cmp(&b.v).unwrap()).unwrap();
    let hi = with.iter().max_by(|a, b| a.v.partial_cmp(&b.v).unwrap()).unwrap();
    let last = with.last().unwrap();
    format!(
        "{title}. {} of {} entries have data. Lowest {} on {}; highest {} on {}; latest {} on {}. Use the arrow keys to hear each value, or open the table.",
        with.len(), past, lo.text, lo.label, hi.text, hi.label, last.text, last.label
    )
}

/// Shared keyboard handling for a chart with `n` points.
fn on_key(n: usize, sel: RwSignal<Option<usize>>) -> impl Fn(leptos::ev::KeyboardEvent) + 'static {
    move |ev: leptos::ev::KeyboardEvent| {
        if n == 0 {
            return;
        }
        let cur = sel.get_untracked().unwrap_or(n - 1);
        let next = match ev.key().as_str() {
            "ArrowLeft" | "ArrowDown" => cur.saturating_sub(1),
            "ArrowRight" | "ArrowUp" => (cur + 1).min(n - 1),
            "Home" => 0,
            "End" => n - 1,
            _ => return,
        };
        ev.prevent_default();
        sel.set(Some(next));
    }
}

fn pointer_index(ev: &leptos::ev::PointerEvent, width: f64, left: f64, n: usize) -> Option<usize> {
    let el = ev.current_target()?.dyn_into::<web_sys::Element>().ok()?;
    let r = el.get_bounding_client_rect();
    let scale = if r.width() > 0.0 { width / r.width() } else { 1.0 };
    let x = (ev.client_x() as f64 - r.left()) * scale - left;
    let slot = (width - left) / n.max(1) as f64;
    Some(((x / slot).floor().max(0.0) as usize).min(n.saturating_sub(1)))
}

/// Default selection: the last point that has data.
fn last_with_data(pts: &[Pt]) -> Option<usize> {
    pts.iter().rposition(|p| p.v.is_some()).or_else(|| pts.iter().rposition(|p| !p.future))
}

#[component]
fn Readout(pts: Arc<Vec<Pt>>, sel: RwSignal<Option<usize>>, #[prop(optional)] id: Option<String>) -> impl IntoView {
    let p2 = pts.clone();
    view! {
        <div class="chart-readout" aria-live="polite" id=id>
            {move || {
                let i = sel.get();
                match i.and_then(|i| p2.get(i)) {
                    Some(p) => view! {
                        <span class="readout-value tabular">{if p.v.is_some() { p.text.clone() } else if p.future { "Not yet".to_string() } else { "No data recorded".to_string() }}</span>
                        <span class="readout-label">{p.label.clone()}</span>
                        {p.note.clone().map(|n| view! { <span class="readout-note">{n}</span> })}
                    }.into_any(),
                    None => view! { <span class="readout-label">"Tap the chart, or focus it and use the arrow keys, to inspect values"</span> }.into_any(),
                }
            }}
        </div>
    }
}

/// Bars for daily totals. `goal` draws a reference line.
#[component]
pub fn BarChart(
    pts: Vec<Pt>,
    tone: &'static str,
    #[prop(into)] title: String,
    #[prop(default = None)] goal: Option<f64>,
    #[prop(default = None)] trend: Option<Vec<Option<f64>>>,
    #[prop(optional)] width: Option<f64>,
    #[prop(optional)] height: Option<f64>,
    #[prop(optional)] baseline_zero_label: Option<&'static str>,
) -> impl IntoView {
    let w = width.unwrap_or(640.0);
    let h = height.unwrap_or(200.0);
    let (left, top, bottom) = (44.0, 12.0, 26.0);
    let n = pts.len();
    let max = pts.iter().filter_map(|p| p.v).chain(trend.iter().flatten().flatten().copied()).fold(goal.unwrap_or(0.0), f64::max).max(1.0);
    let ticks = nice_ticks(0.0, max, 3);
    let ymax = *ticks.last().unwrap_or(&max);
    let plot_h = h - top - bottom;
    let y = move |v: f64| top + plot_h - v / ymax * plot_h;
    let slot = (w - left) / n.max(1) as f64;
    let bw = (slot * 0.62).min(24.0).max(2.0);
    let summary = summarize_pts(&title, &pts);
    let pts = Arc::new(pts);
    let sel = RwSignal::new(last_with_data(&pts));
    let label_every = ((n as f64) / 7.0).ceil().max(1.0) as usize;
    let _ = baseline_zero_label;

    let bars = pts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let cx = left + slot * (i as f64 + 0.5);
            let x0 = cx - bw / 2.0;
            match p.v {
                Some(v) if v > 0.0 => {
                    let yt = y(v);
                    let r = 4.0f64.min(bw / 2.0).min((top + plot_h - yt) / 1.0);
                    let base = top + plot_h;
                    let d = format!(
                        "M{x0:.1},{base:.1}V{:.1}Q{x0:.1},{yt:.1} {:.1},{yt:.1}H{:.1}Q{:.1},{yt:.1} {:.1},{:.1}V{base:.1}Z",
                        yt + r,
                        x0 + r,
                        x0 + bw - r,
                        x0 + bw,
                        x0 + bw,
                        yt + r
                    );
                    let met = goal.map(|g| v >= g).unwrap_or(false);
                    view! { <path d=d class=format!("bar tone-{tone}") class:met=met /> }.into_any()
                }
                Some(_) => view! { <rect x=format!("{:.1}", x0) y=format!("{:.1}", top + plot_h - 1.5) width=format!("{bw:.1}") height="1.5" class=format!("bar tone-{tone}") /> }.into_any(),
                None if p.future => ().into_any(),
                None => view! { <circle cx=format!("{cx:.1}") cy=format!("{:.1}", top + plot_h - 4.0) r="2.5" class="nodata-dot" /> }.into_any(),
            }
        })
        .collect_view();

    let grid = ticks
        .iter()
        .map(|t| {
            let yy = y(*t);
            view! {
                <line x1=format!("{left}") x2=format!("{w}") y1=format!("{yy:.1}") y2=format!("{yy:.1}") class=if *t == 0.0 { "axis" } else { "grid" } />
                <text x=format!("{:.1}", left - 8.0) y=format!("{:.1}", yy + 4.0) class="tick" text-anchor="end">{fmt_tick(*t)}</text>
            }
        })
        .collect_view();
    let xlabels = pts
        .iter()
        .enumerate()
        .filter(|(i, _)| (n - 1 - i) % label_every == 0)
        .map(|(i, p)| view! { <text x=format!("{:.1}", left + slot * (i as f64 + 0.5)) y=format!("{}", h - 6.0) class="tick" text-anchor="middle">{p.tick.clone()}</text> })
        .collect_view();
    let trend_v = trend.as_ref().map(|t| {
        let d = trend_path(t, |i| left + slot * (i as f64 + 0.5), y);
        view! { <path d=d class="trend-line" /> }
    });
    let goal_line = goal.filter(|g| *g <= ymax).map(|g| {
        let yy = y(g);
        view! {
            <line x1=format!("{left}") x2=format!("{w}") y1=format!("{yy:.1}") y2=format!("{yy:.1}") class="goal-line" />
            <text x=format!("{w}") y=format!("{:.1}", yy - 5.0) class="tick goal-label" text-anchor="end">"Goal"</text>
        }
    });
    let pts_r = pts.clone();
    let cross = move || {
        sel.get().map(|i| {
            let cx = left + slot * (i as f64 + 0.5);
            view! { <rect x=format!("{:.1}", cx - slot / 2.0) y=format!("{top}") width=format!("{slot:.1}") height=format!("{plot_h:.1}") class="select-band" /> }
        })
    };
    view! {
        <figure class="chart">
            <Readout pts=pts_r sel=sel />
            <svg
                viewBox=format!("0 0 {w} {h}")
                width="100%"
                class="chart-svg"
                role="application"
                aria-roledescription="chart"
                tabindex="0"
                aria-label=summary.clone()
                on:keydown=on_key(n, sel)
                on:pointermove=move |ev| { if let Some(i) = pointer_index(&ev, w, left, n) { sel.set(Some(i)) } }
                on:pointerdown=move |ev| { if let Some(i) = pointer_index(&ev, w, left, n) { sel.set(Some(i)) } }
            >
                {grid}
                {cross}
                {bars}
                {trend_v}
                {goal_line}
                {xlabels}
            </svg>
        </figure>
    }
}

/// Points for daily levels; consecutive days are joined, gaps are not.
/// `deviation` draws the zero baseline as the reference.
#[component]
pub fn LineChart(
    pts: Vec<Pt>,
    tone: &'static str,
    #[prop(into)] title: String,
    #[prop(optional)] width: Option<f64>,
    #[prop(optional)] height: Option<f64>,
    #[prop(optional)] deviation: bool,
    #[prop(default = None)] trend: Option<Vec<Option<f64>>>,
) -> impl IntoView {
    let w = width.unwrap_or(640.0);
    let h = height.unwrap_or(200.0);
    let (left, top, bottom) = (44.0, 14.0, 26.0);
    let n = pts.len();
    let vals: Vec<f64> = pts.iter().flat_map(|p| [p.v, p.lo, p.hi]).flatten().collect();
    let (mut lo, mut hi) = (vals.iter().cloned().fold(f64::INFINITY, f64::min), vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    if !lo.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    if deviation {
        let m = lo.abs().max(hi.abs()).max(0.5);
        lo = -m;
        hi = m;
    }
    let pad = ((hi - lo) * 0.15).max(1e-6);
    let ticks = nice_ticks(lo - pad, hi + pad, 3);
    let (ymin, ymax) = (*ticks.first().unwrap_or(&lo), *ticks.last().unwrap_or(&hi));
    let plot_h = h - top - bottom;
    let y = move |v: f64| top + plot_h - (v - ymin) / (ymax - ymin).max(1e-9) * plot_h;
    let slot = (w - left) / n.max(1) as f64;
    let xc = move |i: usize| left + slot * (i as f64 + 0.5);
    let summary = summarize_pts(&title, &pts);
    let pts = Arc::new(pts);
    let sel = RwSignal::new(last_with_data(&pts));
    let label_every = ((n as f64) / 7.0).ceil().max(1.0) as usize;

    // Path segments over runs of consecutive points with data.
    let mut segs: Vec<String> = vec![];
    let mut cur = String::new();
    for (i, p) in pts.iter().enumerate() {
        match p.v {
            Some(v) => {
                cur.push_str(&format!("{}{:.1},{:.1}", if cur.is_empty() { "M" } else { "L" }, xc(i), y(v)));
            }
            None => {
                if !cur.is_empty() {
                    segs.push(std::mem::take(&mut cur));
                }
            }
        }
    }
    if !cur.is_empty() {
        segs.push(cur);
    }
    let lines = segs.into_iter().map(|d| view! { <path d=d class=format!("line tone-{tone}") /> }).collect_view();
    let trend_v = trend.as_ref().map(|t| {
        let d = trend_path(t, xc, y);
        view! { <path d=d class="trend-line" /> }
    });
    let show_markers = n <= 62;
    let marks = pts
        .iter()
        .enumerate()
        .map(|(i, p)| match p.v {
            Some(v) => {
                let whisker = match (p.lo, p.hi) {
                    (Some(a), Some(b)) => Some(view! { <line x1=format!("{:.1}", xc(i)) x2=format!("{:.1}", xc(i)) y1=format!("{:.1}", y(a)) y2=format!("{:.1}", y(b)) class=format!("whisker tone-{tone}") /> }),
                    _ => None,
                };
                view! {
                    {whisker}
                    {show_markers.then(|| view! { <circle cx=format!("{:.1}", xc(i)) cy=format!("{:.1}", y(v)) r="4" class=format!("marker tone-{tone}") /> })}
                }
                .into_any()
            }
            None if p.future => ().into_any(),
            None => view! { <circle cx=format!("{:.1}", xc(i)) cy=format!("{:.1}", top + plot_h - 4.0) r="2.5" class="nodata-dot" /> }.into_any(),
        })
        .collect_view();
    let grid = ticks
        .iter()
        .map(|t| {
            let yy = y(*t);
            let cls = if deviation && t.abs() < 1e-9 { "axis" } else { "grid" };
            view! {
                <line x1=format!("{left}") x2=format!("{w}") y1=format!("{yy:.1}") y2=format!("{yy:.1}") class=cls />
                <text x=format!("{:.1}", left - 8.0) y=format!("{:.1}", yy + 4.0) class="tick" text-anchor="end">{if deviation && *t > 0.0 { format!("+{}", fmt_tick(*t)) } else { fmt_tick(*t) }}</text>
            }
        })
        .collect_view();
    let xlabels = pts
        .iter()
        .enumerate()
        .filter(|(i, _)| (n - 1 - i) % label_every == 0)
        .map(|(i, p)| view! { <text x=format!("{:.1}", xc(i)) y=format!("{}", h - 6.0) class="tick" text-anchor="middle">{p.tick.clone()}</text> })
        .collect_view();
    let p3 = pts.clone();
    let cross = move || {
        sel.get().map(|i| {
            let focus = p3.get(i).and_then(|p| p.v).map(|v| view! { <circle cx=format!("{:.1}", xc(i)) cy=format!("{:.1}", y(v)) r="6" class=format!("marker focus tone-{tone}") /> });
            view! {
                <line x1=format!("{:.1}", xc(i)) x2=format!("{:.1}", xc(i)) y1=format!("{top}") y2=format!("{:.1}", top + plot_h) class="crosshair" />
                {focus}
            }
        })
    };
    let pts_r = pts.clone();
    view! {
        <figure class="chart">
            <Readout pts=pts_r sel=sel />
            <svg
                viewBox=format!("0 0 {w} {h}")
                width="100%"
                class="chart-svg"
                role="application"
                aria-roledescription="chart"
                tabindex="0"
                aria-label=summary.clone()
                on:keydown=on_key(n, sel)
                on:pointermove=move |ev| { if let Some(i) = pointer_index(&ev, w, left, n) { sel.set(Some(i)) } }
                on:pointerdown=move |ev| { if let Some(i) = pointer_index(&ev, w, left, n) { sel.set(Some(i)) } }
            >
                {grid}
                {trend_v}
                {lines}
                {marks}
                {cross}
                {xlabels}
            </svg>
        </figure>
    }
}

/// One aggregated bucket for intraday charts (display units).
#[derive(Clone, Debug, PartialEq)]
pub struct IBucket {
    pub t0: i64,
    pub t1: i64,
    pub avg: f64,
    pub min: f64,
    pub max: f64,
    pub label: String,
    pub text: String,
}

/// Intraday chart over `[from, to)`. `bars` = interval totals (steps per
/// hour); otherwise a 2 px average line with a min–max band. Recording gaps
/// are shaded and labelled; lines never bridge them.
#[component]
pub fn IntradayChart(
    buckets: Vec<IBucket>,
    gaps: Vec<(i64, i64)>,
    from: i64,
    to: i64,
    tone: &'static str,
    #[prop(into)] title: String,
    hour_labels: Vec<(i64, String)>,
    #[prop(optional)] bars: bool,
    #[prop(optional)] width: Option<f64>,
    #[prop(optional)] height: Option<f64>,
    #[prop(optional)] max_gap: Option<i64>,
) -> impl IntoView {
    let w = width.unwrap_or(640.0);
    let h = height.unwrap_or(210.0);
    let (left, top, bottom) = (44.0, 12.0, 26.0);
    let plot_h = h - top - bottom;
    let span = (to - from).max(1) as f64;
    let x = move |t: i64| left + (t - from) as f64 / span * (w - left);
    let (lo, hi) = if bars {
        (0.0, buckets.iter().map(|b| b.avg).fold(1.0, f64::max))
    } else {
        let lo = buckets.iter().map(|b| b.min).fold(f64::INFINITY, f64::min);
        let hi = buckets.iter().map(|b| b.max).fold(f64::NEG_INFINITY, f64::max);
        if lo.is_finite() { (lo - 5.0, hi + 5.0) } else { (40.0, 120.0) }
    };
    let ticks = nice_ticks(lo, hi, 3);
    let (ymin, ymax) = (*ticks.first().unwrap_or(&lo), *ticks.last().unwrap_or(&hi));
    let y = move |v: f64| top + plot_h - (v - ymin) / (ymax - ymin).max(1e-9) * plot_h;
    let n = buckets.len();
    let gap_limit = max_gap.unwrap_or(i64::MAX);

    let grid = ticks
        .iter()
        .map(|t| {
            let yy = y(*t);
            view! {
                <line x1=format!("{left}") x2=format!("{w}") y1=format!("{yy:.1}") y2=format!("{yy:.1}") class=if bars && *t == 0.0 { "axis" } else { "grid" } />
                <text x=format!("{:.1}", left - 8.0) y=format!("{:.1}", yy + 4.0) class="tick" text-anchor="end">{fmt_tick(*t)}</text>
            }
        })
        .collect_view();
    let gap_rects = gaps
        .iter()
        .map(|(a, b)| {
            let (xa, xb) = (x(*a), x(*b));
            let wide = xb - xa > 70.0;
            view! {
                <rect x=format!("{xa:.1}") y=format!("{top}") width=format!("{:.1}", (xb - xa).max(1.0)) height=format!("{plot_h:.1}") class="gap-band" />
                {wide.then(|| view! { <text x=format!("{:.1}", (xa + xb) / 2.0) y=format!("{:.1}", top + plot_h / 2.0) class="tick gap-label" text-anchor="middle">"No recording"</text> })}
            }
        })
        .collect_view();
    let marks = if bars {
        buckets
            .iter()
            .map(|b| {
                let (xa, xb) = (x(b.t0), x(b.t1));
                let bw = ((xb - xa) * 0.7).min(24.0).max(1.5);
                let cx = (xa + xb) / 2.0;
                let yt = y(b.avg);
                let base = top + plot_h;
                let r = 3.0f64.min(bw / 2.0).min(base - yt);
                let x0 = cx - bw / 2.0;
                let d = format!(
                    "M{x0:.1},{base:.1}V{:.1}Q{x0:.1},{yt:.1} {:.1},{yt:.1}H{:.1}Q{:.1},{yt:.1} {:.1},{:.1}V{base:.1}Z",
                    yt + r,
                    x0 + r,
                    x0 + bw - r,
                    x0 + bw,
                    x0 + bw,
                    yt + r
                );
                view! { <path d=d class=format!("bar tone-{tone}") /> }
            })
            .collect_view()
            .into_any()
    } else {
        // Split into runs separated by more than `gap_limit`.
        let mut runs: Vec<Vec<&IBucket>> = vec![];
        for b in &buckets {
            match runs.last_mut() {
                Some(r) if b.t0 - r.last().unwrap().t1 <= gap_limit => r.push(b),
                _ => runs.push(vec![b]),
            }
        }
        runs.iter()
            .map(|r| {
                let mid = |b: &IBucket| (b.t0 + b.t1) / 2;
                let line: String = r.iter().enumerate().map(|(i, b)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(mid(b)), y(b.avg))).collect();
                let mut band: String = r.iter().enumerate().map(|(i, b)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(mid(b)), y(b.max))).collect();
                for b in r.iter().rev() {
                    band.push_str(&format!("L{:.1},{:.1}", x(mid(b)), y(b.min)));
                }
                band.push('Z');
                let single = r.len() == 1;
                view! {
                    <path d=band class=format!("band tone-{tone}") />
                    <path d=line class=format!("line tone-{tone}") />
                    {single.then(|| view! { <circle cx=format!("{:.1}", x(mid(r[0]))) cy=format!("{:.1}", y(r[0].avg)) r="3" class=format!("marker tone-{tone}") /> })}
                }
            })
            .collect_view()
            .into_any()
    };
    let xlabels = hour_labels
        .iter()
        .map(|(t, l)| {
            let xx = x(*t);
            let anchor = if xx > w - 20.0 { "end" } else if xx < left + 12.0 { "start" } else { "middle" };
            view! { <text x=format!("{:.1}", xx) y=format!("{}", h - 6.0) class="tick" text-anchor=anchor>{l.clone()}</text> }
        })
        .collect_view();
    let pts: Arc<Vec<Pt>> = Arc::new(
        buckets.iter().map(|b| Pt { label: b.label.clone(), tick: String::new(), v: Some(b.avg), lo: None, hi: None, text: b.text.clone(), note: None, future: false }).collect(),
    );
    let summary = summarize_pts(&title, &pts);
    let sel = RwSignal::new(if n > 0 { Some(n - 1) } else { None });
    let bk = Arc::new(buckets);
    let bk2 = bk.clone();
    let cross = move || {
        sel.get().and_then(|i| bk2.get(i).cloned()).map(|b| {
            let cx = x((b.t0 + b.t1) / 2);
            view! {
                <line x1=format!("{cx:.1}") x2=format!("{cx:.1}") y1=format!("{top}") y2=format!("{:.1}", top + plot_h) class="crosshair" />
                {(!bars).then(|| view! { <circle cx=format!("{cx:.1}") cy=format!("{:.1}", y(b.avg)) r="5" class=format!("marker focus tone-{tone}") /> })}
            }
        })
    };
    let bk3 = bk.clone();
    let pick = move |ev: leptos::ev::PointerEvent| {
        let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        let r = el.get_bounding_client_rect();
        let scale = if r.width() > 0.0 { w / r.width() } else { 1.0 };
        let px = (ev.client_x() as f64 - r.left()) * scale;
        let t = from + ((px - left) / (w - left) * span) as i64;
        if let Some((i, _)) = bk3.iter().enumerate().min_by_key(|(_, b)| ((b.t0 + b.t1) / 2 - t).abs()) {
            sel.set(Some(i));
        }
    };
    let pick2 = pick.clone();
    view! {
        <figure class="chart">
            <Readout pts=pts sel=sel />
            <svg
                viewBox=format!("0 0 {w} {h}")
                width="100%"
                class="chart-svg"
                role="application"
                aria-roledescription="chart"
                tabindex="0"
                aria-label=summary.clone()
                on:keydown=on_key(n, sel)
                on:pointermove=pick
                on:pointerdown=pick2
            >
                {grid}
                {gap_rects}
                {marks}
                {cross}
                {xlabels}
            </svg>
        </figure>
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageSeg {
    pub stage: &'static str,
    pub start: i64,
    pub end: i64,
}

pub const STAGE_ROWS: [(&str, &str); 4] = [("awake", "Awake"), ("rem", "REM"), ("light", "Light"), ("deep", "Deep")];

/// Sleep-stage timeline (hypnogram): one row per stage, so stage is
/// encoded by position as well as color.
#[component]
pub fn SleepTimeline(
    segs: Vec<StageSeg>,
    from: i64,
    to: i64,
    labels: Vec<(i64, String)>,
    #[prop(into)] title: String,
    #[prop(optional)] width: Option<f64>,
    #[prop(optional)] describe: Option<Arc<dyn Fn(&StageSeg) -> String + Send + Sync>>,
) -> impl IntoView {
    let w = width.unwrap_or(640.0);
    let left = 56.0;
    let row_h = 26.0;
    let top = 6.0;
    let h = top + row_h * 4.0 + 26.0;
    let span = (to - from).max(1) as f64;
    let x = move |t: i64| left + (t - from) as f64 / span * (w - left);
    let row = |s: &str| STAGE_ROWS.iter().position(|(k, _)| *k == s).unwrap_or(2);
    let rows = STAGE_ROWS
        .iter()
        .enumerate()
        .map(|(i, (_, l))| {
            let yy = top + row_h * i as f64;
            view! {
                <line x1=format!("{left}") x2=format!("{w}") y1=format!("{:.1}", yy + row_h) y2=format!("{:.1}", yy + row_h) class="grid" />
                <text x="0" y=format!("{:.1}", yy + row_h / 2.0 + 4.0) class="tick stage-label">{*l}</text>
            }
        })
        .collect_view();
    let rects = segs
        .iter()
        .map(|s| {
            let r = row(s.stage);
            let (xa, xb) = (x(s.start), x(s.end));
            view! {
                <rect x=format!("{:.1}", xa) y=format!("{:.1}", top + row_h * r as f64 + 3.0) width=format!("{:.1}", (xb - xa - 1.0).max(1.0)) height=format!("{:.1}", row_h - 6.0) rx="3" class=format!("stage stage-{}", s.stage) />
            }
        })
        .collect_view();
    let xl = labels
        .iter()
        .map(|(t, l)| view! { <text x=format!("{:.1}", x(*t)) y=format!("{}", h - 6.0) class="tick" text-anchor="middle">{l.clone()}</text> })
        .collect_view();
    let n = segs.len();
    let segs = Arc::new(segs);
    let pts: Arc<Vec<Pt>> = Arc::new(
        segs.iter()
            .map(|s| {
                let text = describe.as_ref().map(|f| f(s)).unwrap_or_default();
                Pt { label: STAGE_ROWS[row(s.stage)].1.to_string(), tick: String::new(), v: Some(1.0), lo: None, hi: None, text, note: None, future: false }
            })
            .collect(),
    );
    let summary = format!("{title}. {n} stage intervals; use the arrow keys to hear each one, or open the table.");
    let sel = RwSignal::new(None::<usize>);
    let s2 = segs.clone();
    let focus = move || {
        sel.get().and_then(|i| s2.get(i).cloned()).map(|s| {
            let r = row(s.stage);
            view! { <rect x=format!("{:.1}", x(s.start) - 1.5) y=format!("{:.1}", top + row_h * r as f64 + 1.5) width=format!("{:.1}", (x(s.end) - x(s.start) + 2.0).max(4.0)) height=format!("{:.1}", row_h - 3.0) rx="4" class="stage-focus" /> }
        })
    };
    let s3 = segs.clone();
    let pick = move |ev: leptos::ev::PointerEvent| {
        let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        let r = el.get_bounding_client_rect();
        let scale = if r.width() > 0.0 { w / r.width() } else { 1.0 };
        let px = (ev.client_x() as f64 - r.left()) * scale;
        let t = from + ((px - left) / (w - left) * span) as i64;
        if let Some(i) = s3.iter().position(|s| s.start <= t && t < s.end) {
            sel.set(Some(i));
        }
    };
    view! {
        <figure class="chart">
            <Readout pts=pts sel=sel />
            <svg
                viewBox=format!("0 0 {w} {h}")
                width="100%"
                class="chart-svg"
                role="application"
                aria-roledescription="chart"
                tabindex="0"
                aria-label=summary.clone()
                on:keydown=on_key(n, sel)
                on:pointerdown=pick
            >
                {rows}
                {rects}
                {focus}
                {xl}
            </svg>
        </figure>
    }
}

/// Compact trend line; missing days break the line.
#[component]
pub fn Sparkline(values: Vec<Option<f64>>, tone: &'static str, #[prop(optional)] width: Option<f64>, #[prop(optional)] height: Option<f64>) -> impl IntoView {
    let w = width.unwrap_or(96.0);
    let h = height.unwrap_or(32.0);
    let vals: Vec<f64> = values.iter().flatten().copied().collect();
    let (lo, hi) = (vals.iter().cloned().fold(f64::INFINITY, f64::min), vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let n = values.len().max(2);
    let x = |i: usize| 3.0 + i as f64 / (n - 1) as f64 * (w - 6.0);
    let y = |v: f64| if hi > lo { h - 4.0 - (v - lo) / (hi - lo) * (h - 8.0) } else { h / 2.0 };
    let mut d = String::new();
    let mut prev = false;
    for (i, v) in values.iter().enumerate() {
        match v {
            Some(v) => {
                d.push_str(&format!("{}{:.1},{:.1}", if prev { "L" } else { "M" }, x(i), y(*v)));
                prev = true;
            }
            None => prev = false,
        }
    }
    let last = values.iter().enumerate().rev().find_map(|(i, v)| v.map(|v| (i, v)));
    view! {
        <svg class="sparkline" viewBox=format!("0 0 {w} {h}") width=format!("{w}") height=format!("{h}") aria-hidden="true" focusable="false">
            <path d=d class=format!("line thin tone-{tone}") />
            {last.map(|(i, v)| view! { <circle cx=format!("{:.1}", x(i)) cy=format!("{:.1}", y(v)) r="3" class=format!("marker tone-{tone}") /> })}
        </svg>
    }
}

/// Progress ring (for the step goal). Decorative; the value is in text.
#[component]
pub fn Ring(fraction: f64, tone: &'static str, #[prop(optional)] size: Option<f64>) -> impl IntoView {
    let s = size.unwrap_or(120.0);
    let stroke = 11.0;
    let r = (s - stroke) / 2.0;
    let c = std::f64::consts::TAU * r;
    let f = fraction.clamp(0.0, 1.0);
    view! {
        <svg class="ring" viewBox=format!("0 0 {s} {s}") width=format!("{s}") height=format!("{s}") aria-hidden="true" focusable="false">
            <circle cx=format!("{}", s / 2.0) cy=format!("{}", s / 2.0) r=format!("{r}") class="ring-track" stroke-width=format!("{stroke}") fill="none" />
            <circle
                cx=format!("{}", s / 2.0)
                cy=format!("{}", s / 2.0)
                r=format!("{r}")
                class=format!("ring-fill tone-{tone}")
                stroke-width=format!("{stroke}")
                fill="none"
                stroke-linecap="round"
                stroke-dasharray=format!("{:.2} {:.2}", c * f, c)
                transform=format!("rotate(-90 {} {})", s / 2.0, s / 2.0)
            />
        </svg>
    }
}

/// Table alternative for any chart.
#[component]
pub fn DataTable(#[prop(into)] caption: String, headers: Vec<&'static str>, rows: Vec<Vec<String>>) -> impl IntoView {
    let label = caption.clone();
    view! {
        <div class="table-wrap" tabindex="0" role="region" aria-label=label>
            <table class="data-table">
                <caption>{caption}</caption>
                <thead><tr>{headers.into_iter().map(|h| view! { <th scope="col">{h}</th> }).collect_view()}</tr></thead>
                <tbody>
                    {rows.into_iter().map(|r| view! {
                        <tr>{r.into_iter().enumerate().map(|(i, c)| if i == 0 { view! { <th scope="row">{c}</th> }.into_any() } else { view! { <td class="tabular">{c}</td> }.into_any() }).collect_view()}</tr>
                    }).collect_view()}
                </tbody>
            </table>
        </div>
    }
}
