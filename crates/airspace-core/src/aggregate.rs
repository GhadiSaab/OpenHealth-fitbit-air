//! Daily summaries derived from raw measurements.
//!
//! Summaries are *derived* values: they carry [`CALC_VERSION`] and a
//! plain-language `method`. Raw measurements are never modified.
//!
//! ## Additive metrics (steps, distance, calories)
//!
//! Sources are ranked; each contributes only for the time not already
//! covered by a higher-ranked source. This prevents double counting when
//! imported history and live Bluetooth recording overlap, while still
//! combining "history until the last sync" with "live since then".
//!
//! | rank | source                               |
//! |------|--------------------------------------|
//! | 0    | imported daily total (whole day)     |
//! | 1    | imported interval counts             |
//! | 2    | live interval counts                 |
//! | 3    | live cumulative counter (deltas)     |
//! | 4    | imported cumulative counter (deltas) |
//!
//! Cumulative counters are converted to increments by differencing
//! consecutive readings. A decrease is treated as a counter reset: the new
//! reading counts from zero. Counter readings are never summed.

use crate::model::*;
use crate::time::{self, MINUTE_MS};
use chrono::NaiveDate;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Version of the summary calculations. Stored with each summary so stale
/// summaries can be recomputed after an upgrade.
pub const CALC_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DaySummary {
    pub day: String,
    pub metric: Metric,
    /// The day's primary value in the metric's canonical unit. `None` means
    /// no data — never zero.
    pub value: Option<f64>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    /// Number of raw records that contributed.
    pub count: u32,
    /// Fraction (0–1) of the day with data, for metrics where that is meaningful.
    #[serde(default)]
    pub coverage: Option<f64>,
    /// Plain-language description of how the value was produced.
    pub method: String,
    pub origins: Vec<Origin>,
    /// Latest timestamp among contributing records.
    #[serde(default)]
    pub latest_ts: Option<i64>,
    #[serde(default)]
    pub provisional: bool,
    #[serde(default)]
    pub extra: Option<SummaryExtra>,
    pub calc_version: u32,
    #[serde(default)]
    pub demo: bool,
}

impl DaySummary {
    pub fn key(&self) -> String {
        summary_key(self.metric, &self.day)
    }
}

pub fn summary_key(metric: Metric, day: &str) -> String {
    format!("{}|{}", metric.key(), day)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SummaryExtra {
    Sleep {
        bed: i64,
        wake: i64,
        /// Minutes in each stage, calculated from stage intervals.
        stage_minutes: BTreeMap<String, f64>,
        stages_measured: bool,
        asleep_derived: bool,
        naps: u32,
        nap_minutes: f64,
        time_in_bed_min: f64,
    },
    Zones {
        minutes: BTreeMap<String, f64>,
        bounds: BTreeMap<String, (Option<f64>, Option<f64>)>,
        origin: ZoneOrigin,
    },
    Hrv {
        method: HrvMethod,
        /// Records measured with other methods, excluded from this value.
        excluded_other_methods: u32,
    },
    Respiratory {
        by_stage: BTreeMap<String, f64>,
    },
    Additive {
        /// Contributions per source, in display order.
        parts: Vec<AdditivePart>,
        /// Records ignored because a higher-ranked source covered the time.
        overlapped: u32,
        /// Counter jumps rejected as implausible.
        rejected_jumps: u32,
        /// Calories only: which energy figure `value` represents.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        calorie_scope: Option<CalorieScope>,
        /// Calories only: active calories when reported separately.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        active: Option<f64>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdditivePart {
    pub label: String,
    pub value: f64,
    pub from: i64,
    pub to: i64,
}

/// Inputs needed to compute summaries.
#[derive(Clone, Debug)]
pub struct SummaryContext {
    pub tz: Tz,
    pub now: i64,
    pub demo: bool,
}

/// Groups measurements by the local day they belong to.
pub fn day_of(m: &Measurement, tz: &Tz) -> NaiveDate {
    match m.kind {
        SampleKind::DailyTotal => m
            .day
            .as_deref()
            .and_then(time::parse_day)
            .unwrap_or_else(|| time::local_day(m.start, m.utc_offset_min, tz)),
        // Sleep belongs to the day you wake up.
        SampleKind::Session => time::local_day(m.end_or_start() - 1, m.utc_offset_min, tz),
        // Intervals belong to the day they start.
        _ => time::local_day(m.start, m.utc_offset_min, tz),
    }
}

type CounterKey = (Metric, Origin, String, bool);

fn resets_daily(m: &Measurement) -> bool {
    matches!(m.detail, Some(Detail::Counter { resets_daily: true }))
}

/// The neighbouring counter readings just before and just after a day, for
/// each counter series. They let an increment that spans midnight be shared
/// between the two days instead of being lost.
fn counter_context<'a>(series: &HashMap<CounterKey, Vec<&'a Measurement>>, metric: Metric, day: NaiveDate, tz: &Tz) -> Vec<&'a Measurement> {
    let (ds, de) = time::day_bounds(day, tz);
    let mut out = vec![];
    for ((m, _, _, _), v) in series {
        if *m != metric {
            continue;
        }
        let i = v.partition_point(|x| x.start < ds);
        if i > 0 {
            out.push(v[i - 1]);
        }
        let j = v.partition_point(|x| x.start < de);
        if j < v.len() {
            out.push(v[j]);
        }
    }
    out
}

fn counter_series<'a>(items: &[&'a Measurement]) -> HashMap<CounterKey, Vec<&'a Measurement>> {
    let mut series: HashMap<CounterKey, Vec<&'a Measurement>> = HashMap::new();
    for m in items.iter().copied().filter(|m| m.kind == SampleKind::CumulativeCounter) {
        series.entry((m.metric, m.prov.origin, m.prov.source_name.clone(), resets_daily(m))).or_default().push(m);
    }
    for v in series.values_mut() {
        v.sort_by_key(|m| (m.start, m.prov.ingested_at));
    }
    series
}

/// Computes summaries for every (metric, day) present in `items`.
pub fn summarize(items: &[Measurement], ctx: &SummaryContext) -> Vec<DaySummary> {
    let active: Vec<&Measurement> = items.iter().filter(|m| m.is_active()).collect();
    let mut groups: BTreeMap<(Metric, NaiveDate), Vec<&Measurement>> = BTreeMap::new();
    for m in &active {
        groups.entry((m.metric, day_of(m, &ctx.tz))).or_default().push(m);
    }
    let series = counter_series(&active);
    groups
        .into_iter()
        .filter_map(|((metric, day), ms)| {
            let context = if metric.is_additive() { counter_context(&series, metric, day, &ctx.tz) } else { vec![] };
            summarize_day_with(metric, day, &ms, &context, ctx)
        })
        .collect()
}

pub fn summarize_day(metric: Metric, day: NaiveDate, ms: &[&Measurement], ctx: &SummaryContext) -> Option<DaySummary> {
    summarize_day_with(metric, day, ms, &[], ctx)
}

/// Like [`summarize_day`], with neighbouring counter readings from adjacent days.
pub fn summarize_day_with(metric: Metric, day: NaiveDate, ms: &[&Measurement], context: &[&Measurement], ctx: &SummaryContext) -> Option<DaySummary> {
    if ms.is_empty() {
        return None;
    }
    let mut s = DaySummary {
        day: time::day_key(day),
        metric,
        value: None,
        min: None,
        max: None,
        count: ms.len() as u32,
        coverage: None,
        method: String::new(),
        origins: unique_origins(ms),
        // For daily totals and intervals the end is exclusive (often next midnight);
        // show the last moment actually covered instead.
        latest_ts: ms.iter().map(|m| match m.kind {
            SampleKind::DailyTotal => m.start,
            SampleKind::Interval => m.end_or_start() - 1,
            _ => m.end_or_start(),
        }).max(),
        provisional: ms.iter().any(|m| m.prov.provisional),
        extra: None,
        calc_version: CALC_VERSION,
        demo: ctx.demo,
    };
    match metric {
        Metric::HeartRate => heart_rate(&mut s, day, ms, ctx),
        Metric::Steps | Metric::Distance => {
            additive_with(&mut s, day, ms, context, ctx);
        }
        Metric::Calories => calories(&mut s, day, ms, context, ctx),
        Metric::Sleep => sleep(&mut s, ms),
        Metric::HeartRateZones => zones(&mut s, ms),
        Metric::Hrv => hrv(&mut s, ms),
        Metric::RespiratoryRate => respiratory(&mut s, ms),
        Metric::RestingHeartRate | Metric::Spo2 | Metric::SkinTemperature => daily_point(&mut s, ms),
    }
    Some(s)
}

fn unique_origins(ms: &[&Measurement]) -> Vec<Origin> {
    let mut v: Vec<Origin> = ms.iter().map(|m| m.prov.origin).collect::<HashSet<_>>().into_iter().collect();
    v.sort();
    v
}

fn mean(v: &[f64]) -> Option<f64> {
    if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) }
}

fn heart_rate(s: &mut DaySummary, day: NaiveDate, ms: &[&Measurement], ctx: &SummaryContext) {
    let vals: Vec<f64> = ms.iter().filter(|m| m.kind == SampleKind::Instant).map(|m| m.value).collect();
    s.value = mean(&vals);
    s.min = vals.iter().cloned().reduce(f64::min);
    s.max = vals.iter().cloned().reduce(f64::max);
    let minutes: HashSet<i64> = ms.iter().map(|m| m.start / MINUTE_MS).collect();
    let (ds, de) = time::day_bounds(day, &ctx.tz);
    let span_end = de.min(ctx.now.max(ds + MINUTE_MS));
    let possible = ((span_end - ds) / MINUTE_MS).max(1) as f64;
    s.coverage = Some((minutes.len() as f64 / possible).min(1.0));
    s.method = "Average of all heart-rate samples recorded this day".into();
}

fn daily_point(s: &mut DaySummary, ms: &[&Measurement]) {
    let vals: Vec<f64> = ms.iter().map(|m| m.value).collect();
    s.value = mean(&vals);
    let (mut lo, mut hi): (Vec<f64>, Vec<f64>) = (vec![], vec![]);
    for m in ms {
        if let Some(Detail::Range { min, max }) = m.detail {
            lo.push(min);
            hi.push(max);
        } else {
            lo.push(m.value);
            hi.push(m.value);
        }
    }
    s.min = lo.into_iter().reduce(f64::min);
    s.max = hi.into_iter().reduce(f64::max);
    s.method = if ms.len() == 1 {
        "Value as recorded by the source".into()
    } else {
        format!("Average of {} recorded values", ms.len())
    };
}

fn hrv(s: &mut DaySummary, ms: &[&Measurement]) {
    // Choose one method; never average RMSSD with SDNN.
    let mut by: HashMap<HrvMethod, Vec<f64>> = HashMap::new();
    for m in ms {
        by.entry(m.hrv_method()).or_default().push(m.value);
    }
    let method = if by.contains_key(&HrvMethod::Rmssd) {
        HrvMethod::Rmssd
    } else {
        *by.iter().max_by_key(|(_, v)| v.len()).map(|(k, _)| k).unwrap_or(&HrvMethod::Unknown)
    };
    let vals = by.remove(&method).unwrap_or_default();
    let excluded: usize = by.values().map(|v| v.len()).sum();
    s.value = mean(&vals);
    s.min = vals.iter().cloned().reduce(f64::min);
    s.max = vals.iter().cloned().reduce(f64::max);
    s.count = vals.len() as u32;
    s.method = format!("{} as recorded by the source", method.label());
    s.extra = Some(SummaryExtra::Hrv { method, excluded_other_methods: excluded as u32 });
}

fn respiratory(s: &mut DaySummary, ms: &[&Measurement]) {
    let mut overall = vec![];
    let mut by_stage: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for m in ms {
        match &m.detail {
            Some(Detail::Respiratory { stage: Some(st) }) => by_stage.entry(st.key().into()).or_default().push(m.value),
            _ => overall.push(m.value),
        }
    }
    s.value = mean(&overall);
    s.min = overall.iter().cloned().reduce(f64::min);
    s.max = overall.iter().cloned().reduce(f64::max);
    s.method = "Overnight average as recorded by the source".into();
    let by_stage: BTreeMap<String, f64> = by_stage.into_iter().filter_map(|(k, v)| mean(&v).map(|m| (k, m))).collect();
    if !by_stage.is_empty() {
        s.extra = Some(SummaryExtra::Respiratory { by_stage });
    }
}

fn zones(s: &mut DaySummary, ms: &[&Measurement]) {
    let mut minutes: BTreeMap<String, f64> = BTreeMap::new();
    let mut bounds = BTreeMap::new();
    let mut origin = ZoneOrigin::Imported;
    for m in ms {
        if let Some(Detail::Zone { zone, lower_bpm, upper_bpm, origin: o }) = &m.detail {
            *minutes.entry(zone_key(*zone).into()).or_default() += m.value;
            bounds.insert(zone_key(*zone).to_string(), (*lower_bpm, *upper_bpm));
            if *o == ZoneOrigin::Calculated {
                origin = ZoneOrigin::Calculated;
            }
        }
    }
    let active: f64 = minutes.iter().filter(|(k, _)| k.as_str() != "below_zones").map(|(_, v)| v).sum();
    s.value = Some(active);
    s.method = match origin {
        ZoneOrigin::Imported => "Minutes in fat burn, cardio and peak zones, as imported".into(),
        ZoneOrigin::Calculated => "Minutes above the fat-burn threshold, calculated by Airspace".into(),
    };
    s.extra = Some(SummaryExtra::Zones { minutes, bounds, origin });
}

pub fn zone_key(z: HrZone) -> &'static str {
    match z {
        HrZone::BelowZones => "below_zones",
        HrZone::FatBurn => "fat_burn",
        HrZone::Cardio => "cardio",
        HrZone::Peak => "peak",
    }
}

fn sleep(s: &mut DaySummary, ms: &[&Measurement]) {
    let sessions: Vec<&&Measurement> = ms.iter().filter(|m| m.kind == SampleKind::Session).collect();
    let main = sessions
        .iter()
        .filter(|m| m.sleep().map(|d| d.is_main).unwrap_or(true))
        .max_by_key(|m| m.end_or_start() - m.start)
        .or_else(|| sessions.iter().max_by_key(|m| m.end_or_start() - m.start));
    let Some(main) = main else {
        s.method = "No sleep session".into();
        return;
    };
    let detail = main.sleep();
    let mut stage_minutes: BTreeMap<String, f64> = BTreeMap::new();
    if let Some(d) = detail {
        for st in &d.stages {
            *stage_minutes.entry(st.stage.key().into()).or_default() += (st.end - st.start) as f64 / MINUTE_MS as f64;
        }
    }
    // Other sessions overlapping the main one are alternate versions of the same
    // night (e.g. "keep both" on import), not naps.
    let naps: Vec<&&&Measurement> = sessions.iter().filter(|m| m.id != main.id && (m.end_or_start() <= main.start || m.start >= main.end_or_start())).collect();
    s.value = Some(main.value);
    s.count = sessions.len() as u32;
    s.method = if detail.map(|d| d.asleep_minutes_derived).unwrap_or(false) {
        "Time asleep in your main sleep, calculated from stage intervals".into()
    } else {
        "Time asleep in your main sleep, as reported by the source".into()
    };
    s.extra = Some(SummaryExtra::Sleep {
        bed: main.start,
        wake: main.end_or_start(),
        stage_minutes,
        stages_measured: detail.map(|d| d.stages_measured).unwrap_or(false),
        asleep_derived: detail.map(|d| d.asleep_minutes_derived).unwrap_or(false),
        naps: naps.len() as u32,
        nap_minutes: naps.iter().map(|m| m.value).sum(),
        time_in_bed_min: (main.end_or_start() - main.start) as f64 / MINUTE_MS as f64,
    });
}

fn calories(s: &mut DaySummary, day: NaiveDate, ms: &[&Measurement], context: &[&Measurement], ctx: &SummaryContext) {
    let total: Vec<&Measurement> = ms.iter().copied().filter(|m| m.calorie_scope() == CalorieScope::Total).collect();
    let active: Vec<&Measurement> = ms.iter().copied().filter(|m| m.calorie_scope() == CalorieScope::Active).collect();
    let unspecified: Vec<&Measurement> =
        ms.iter().copied().filter(|m| m.calorie_scope() == CalorieScope::Unspecified).collect();
    let active_total = if active.is_empty() {
        None
    } else {
        let mut tmp = s.clone();
        let cx: Vec<&Measurement> = context.iter().copied().filter(|m| m.calorie_scope() == CalorieScope::Active).collect();
        additive_with(&mut tmp, day, &active, &cx, ctx);
        tmp.value
    };
    let (scope, chosen) = if !total.is_empty() {
        (CalorieScope::Total, total)
    } else if !unspecified.is_empty() {
        (CalorieScope::Unspecified, unspecified)
    } else {
        (CalorieScope::Active, active)
    };
    let cx: Vec<&Measurement> = context.iter().copied().filter(|m| m.calorie_scope() == scope).collect();
    additive_with(s, day, &chosen, &cx, ctx);
    s.method = match scope {
        CalorieScope::Total => format!("Total calories (including resting energy). {}", s.method),
        CalorieScope::Active => format!("Active calories only. {}", s.method),
        CalorieScope::Unspecified => format!("Calories; the source does not say whether this is total or active. {}", s.method),
    };
    if let Some(SummaryExtra::Additive { calorie_scope, active, .. }) = &mut s.extra {
        *calorie_scope = Some(scope);
        *active = if scope == CalorieScope::Active { None } else { active_total };
    }
}

/// Rank of a (origin, kind) source for additive metrics; lower wins.
fn rank(origin: Origin, kind: SampleKind) -> u8 {
    match (origin, kind) {
        (Origin::HistoryImport, SampleKind::DailyTotal) => 0,
        (Origin::HistoryImport, SampleKind::Interval) => 1,
        (Origin::BluetoothLive, SampleKind::Interval) => 2,
        (Origin::BluetoothLive, SampleKind::CumulativeCounter) => 3,
        (Origin::HistoryImport, SampleKind::CumulativeCounter) => 4,
        (Origin::BluetoothLive, SampleKind::DailyTotal) => 5,
        _ => 9,
    }
}

fn origin_label(o: Origin) -> &'static str {
    match o {
        Origin::HistoryImport => "Imported",
        Origin::BluetoothLive => "Bluetooth",
        Origin::Derived => "Derived",
    }
}

/// Covered time spans, used to keep lower-ranked sources out of time that a
/// higher-ranked source already accounts for.
#[derive(Default)]
struct Coverage(Vec<(i64, i64)>);

impl Coverage {
    fn overlaps(&self, a: i64, b: i64) -> bool {
        self.0.iter().any(|&(s, e)| a < e && b > s)
    }
    /// How much of `[a, b)` is not yet covered.
    fn uncovered_len(&self, a: i64, b: i64) -> i64 {
        (b - a) - union_len(self.0.clone(), a, b)
    }
    fn add(&mut self, a: i64, b: i64) {
        if b > a {
            self.0.push((a, b));
        }
    }
}

/// Maximum plausible increments per minute, used to reject corrupt counter jumps.
fn max_per_minute(metric: Metric) -> f64 {
    match metric {
        Metric::Steps => 300.0,
        Metric::Distance => 400.0,
        Metric::Calories => 40.0,
        _ => f64::INFINITY,
    }
}

/// One accepted increment of an additive metric: `[from, to)` and amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Increment {
    pub from: i64,
    pub to: i64,
    pub value: f64,
}

/// The increments that make up a day's additive total, after source
/// precedence and overlap removal. Used for intraday charts so they always
/// agree with the daily total.
pub fn additive_increments(metric: Metric, day: NaiveDate, ms: &[&Measurement], ctx: &SummaryContext) -> Vec<Increment> {
    let mut s = DaySummary {
        day: time::day_key(day),
        metric,
        value: None,
        min: None,
        max: None,
        count: 0,
        coverage: None,
        method: String::new(),
        origins: vec![],
        latest_ts: None,
        provisional: false,
        extra: None,
        calc_version: CALC_VERSION,
        demo: ctx.demo,
    };
    let ms: Vec<&Measurement> = if metric == Metric::Calories {
        // Mirror the scope choice made for the daily total.
        let in_day: Vec<&Measurement> = ms.iter().copied().filter(|m| day_of(m, &ctx.tz) == day).collect();
        let has = |sc: CalorieScope| in_day.iter().any(|m| m.calorie_scope() == sc);
        let sc = if has(CalorieScope::Total) { CalorieScope::Total } else if has(CalorieScope::Unspecified) { CalorieScope::Unspecified } else { CalorieScope::Active };
        ms.iter().copied().filter(|m| m.calorie_scope() == sc).collect()
    } else {
        ms.to_vec()
    };
    // Records outside the day only serve as counter context.
    let all: Vec<&Measurement> = ms.iter().copied().filter(|m| m.is_active()).collect();
    let in_day: Vec<&Measurement> = all.iter().copied().filter(|m| day_of(m, &ctx.tz) == day).collect();
    let context = counter_context(&counter_series(&all), metric, day, &ctx.tz);
    additive_with(&mut s, day, &in_day, &context, ctx)
}

/// Total length of the union of intervals, clipped to `[lo, hi)`.
fn union_len(mut v: Vec<(i64, i64)>, lo: i64, hi: i64) -> i64 {
    v.sort_unstable();
    let mut total = 0;
    let mut cur: Option<(i64, i64)> = None;
    for (a, b) in v {
        let (a, b) = (a.max(lo), b.min(hi));
        if b <= a {
            continue;
        }
        match cur {
            Some((x, y)) if a <= y => cur = Some((x, y.max(b))),
            Some((x, y)) => {
                total += y - x;
                cur = Some((a, b));
            }
            None => cur = Some((a, b)),
        }
    }
    if let Some((x, y)) = cur {
        total += y - x;
    }
    total
}

fn additive_with(s: &mut DaySummary, day: NaiveDate, ms: &[&Measurement], context: &[&Measurement], ctx: &SummaryContext) -> Vec<Increment> {
    let mut incs = vec![];
    let (ds, de) = time::day_bounds(day, &ctx.tz);
    let mut groups: BTreeMap<(u8, String), Vec<&Measurement>> = BTreeMap::new();
    for m in ms {
        groups.entry((rank(m.prov.origin, m.kind), m.prov.source_name.clone())).or_default().push(m);
    }
    let mut all_intervals: Vec<(i64, i64)> = vec![];
    let mut covered = Coverage::default();
    let mut parts = vec![];
    let mut overlapped = 0u32;
    let mut rejected = 0u32;
    for ((_, source), mut g) in groups {
        let kind = g[0].kind;
        let origin = g[0].prov.origin;
        g.sort_by_key(|m| (m.start, m.prov.ingested_at));
        let mut total = 0.0;
        // The exact intervals this source contributed (coverage uses these, not the span).
        let mut span: Vec<(i64, i64)> = vec![];
        let extend = |a: i64, b: i64, span: &mut Vec<(i64, i64)>| span.push((a, b));
        let mut contributed = false;
        match kind {
            SampleKind::DailyTotal => {
                if covered.overlaps(ds, de) {
                    overlapped += g.len() as u32;
                    continue;
                }
                // Several daily totals from one source: use the most recently ingested.
                let latest = g.iter().max_by_key(|m| m.prov.ingested_at).unwrap();
                total = latest.value;
                incs.push(Increment { from: ds, to: de, value: total });
                overlapped += g.len() as u32 - 1;
                extend(ds, de, &mut span);
                contributed = true;
            }
            SampleKind::Interval => {
                let mut last_end = i64::MIN;
                for m in &g {
                    let (a, b) = (m.start, m.end.unwrap_or(m.start + MINUTE_MS));
                    // Overlap within the same source (e.g. 1-min and 15-min series) or
                    // with a higher-ranked source: skip.
                    if a < last_end || covered.overlaps(a, b) {
                        overlapped += 1;
                        continue;
                    }
                    total += m.value;
                    incs.push(Increment { from: a, to: b, value: m.value });
                    last_end = b;
                    extend(a, b, &mut span);
                    contributed = true;
                }
            }
            SampleKind::CumulativeCounter => {
                let resets_daily = g.iter().any(|m| resets_daily(m));
                let limit = max_per_minute(s.metric);
                // A counter that resets at midnight starts from zero each day. Otherwise
                // include the readings just before and after the day, so an increment
                // spanning midnight is shared between the days instead of lost.
                if !resets_daily {
                    for c in context.iter().filter(|c| c.kind == SampleKind::CumulativeCounter && c.prov.source_name == source && c.prov.origin == origin && !self::resets_daily(c)) {
                        if !g.iter().any(|m| m.id == c.id) {
                            g.push(c);
                        }
                    }
                    g.sort_by_key(|m| (m.start, m.prov.ingested_at));
                }
                let mut prev: Option<&Measurement> = None;
                for m in &g {
                    let (a, inc) = match prev {
                        None => {
                            prev = Some(m);
                            if resets_daily && m.start >= ds && m.start < de {
                                (ds, m.value)
                            } else {
                                continue;
                            }
                        }
                        Some(p) => {
                            let d = m.value - p.value;
                            prev = Some(m);
                            if d < 0.0 {
                                // A reset: the new reading counts from zero. Devices reset at
                                // midnight, so never spread it back into an earlier day.
                                (p.start.max(time::day_start(time::local_day(m.start, m.utc_offset_min, &ctx.tz), &ctx.tz)), m.value)
                            } else {
                                (p.start, d)
                            }
                        }
                    };
                    let b = m.start.max(a);
                    let mins = ((b - a) as f64 / MINUTE_MS as f64).max(1.0);
                    if inc / mins > limit {
                        rejected += 1;
                        continue;
                    }
                    // The share of this increment inside the day, pro rata by time.
                    let (ca, cb) = (a.max(ds), b.min(de));
                    if cb <= ca {
                        continue;
                    }
                    let free = covered.uncovered_len(ca, cb);
                    if free <= 0 {
                        overlapped += 1;
                        continue;
                    }
                    // Only the part not already accounted for by a higher-ranked source counts.
                    let share = inc * free as f64 / (b - a).max(1) as f64;
                    total += share;
                    incs.push(Increment { from: ca, to: cb, value: share });
                    extend(ca, cb, &mut span);
                    contributed = true;
                }
            }
            _ => continue,
        }
        if contributed {
            let a = span.iter().map(|u| u.0).min().unwrap();
            let b = span.iter().map(|u| u.1).max().unwrap();
            for (x, y) in &span {
                covered.add(*x, *y);
            }
            all_intervals.extend(span.iter().copied());
            let what = match kind {
                SampleKind::DailyTotal => "daily total",
                SampleKind::Interval => "interval counts",
                SampleKind::CumulativeCounter => "counter changes",
                _ => "",
            };
            parts.push(AdditivePart { label: format!("{} {} ({})", origin_label(origin), what, source), value: total, from: a, to: b });
        }
    }
    if parts.is_empty() {
        s.value = None;
        s.method = "No usable records".into();
    } else {
        s.value = Some(parts.iter().map(|p| p.value).sum());
        let covered_ms = union_len(all_intervals, ds, de);
        s.coverage = Some((covered_ms as f64 / (de - ds) as f64).min(1.0));
        s.method = if parts.len() == 1 {
            format!("From {}", parts[0].label.to_lowercase())
        } else {
            format!(
                "Combined from {} sources without overlap: {}",
                parts.len(),
                parts.iter().map(|p| p.label.to_lowercase()).collect::<Vec<_>>().join(", then ")
            )
        };
    }
    s.extra = Some(SummaryExtra::Additive { parts, overlapped, rejected_jumps: rejected, calorie_scope: None, active: None });
    incs
}

/// A measurement variant that must not be mixed with others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Hrv(HrvMethod),
    Calories(CalorieScope),
}

impl Variant {
    pub fn label(self) -> &'static str {
        match self {
            Variant::Hrv(m) => m.label(),
            Variant::Calories(CalorieScope::Total) => "total calories",
            Variant::Calories(CalorieScope::Active) => "active calories",
            Variant::Calories(CalorieScope::Unspecified) => "calories of unspecified type",
        }
    }
}

/// The variant a day's summary was computed with, if the metric has variants.
pub fn variant_of(s: &DaySummary) -> Option<Variant> {
    match &s.extra {
        Some(SummaryExtra::Hrv { method, .. }) => Some(Variant::Hrv(*method)),
        Some(SummaryExtra::Additive { calorie_scope: Some(sc), .. }) => Some(Variant::Calories(*sc)),
        _ => None,
    }
}

/// In-memory index of summaries for quick lookups by the UI.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SummaryIndex {
    pub map: BTreeMap<(Metric, NaiveDate), DaySummary>,
}

impl SummaryIndex {
    pub fn from_vec(v: Vec<DaySummary>) -> Self {
        let mut idx = SummaryIndex::default();
        idx.extend(v);
        idx
    }
    pub fn extend(&mut self, v: Vec<DaySummary>) {
        for s in v {
            if let Some(d) = time::parse_day(&s.day) {
                self.map.insert((s.metric, d), s);
            }
        }
    }
    /// Removes summaries for the given (metric, day) slots.
    pub fn remove(&mut self, keys: &[(Metric, NaiveDate)]) {
        for k in keys {
            self.map.remove(k);
        }
    }
    pub fn get(&self, metric: Metric, day: NaiveDate) -> Option<&DaySummary> {
        self.map.get(&(metric, day))
    }
    pub fn value(&self, metric: Metric, day: NaiveDate) -> Option<f64> {
        self.get(metric, day).and_then(|s| s.value)
    }
    /// Values for each day in `[from, to]`; missing days are `None`.
    pub fn series(&self, metric: Metric, from: NaiveDate, to: NaiveDate) -> Vec<(NaiveDate, Option<f64>)> {
        time::days_inclusive(from, to).map(|d| (d, self.value(metric, d))).collect()
    }
    /// Values for `[from, to]` restricted to one measurement variant — the HRV
    /// method or calorie type used by most days — so incompatible values are
    /// never averaged or compared. Returns the series, the variant, and how many
    /// days were left out.
    pub fn consistent_series(&self, metric: Metric, from: NaiveDate, to: NaiveDate) -> (Vec<(NaiveDate, Option<f64>)>, Option<Variant>, usize) {
        let variant = self.majority_variant(metric, from, to);
        let mut excluded = 0;
        let v = time::days_inclusive(from, to)
            .map(|d| {
                let s = self.get(metric, d);
                let val = s.and_then(|s| s.value);
                match (variant, s.and_then(variant_of)) {
                    (Some(want), Some(have)) if want != have && val.is_some() => {
                        excluded += 1;
                        (d, None)
                    }
                    _ => (d, val),
                }
            })
            .collect();
        (v, variant, excluded)
    }

    /// The HRV method or calorie type used by most days with data in `[from, to]`
    /// (RMSSD and total calories win ties).
    pub fn majority_variant(&self, metric: Metric, from: NaiveDate, to: NaiveDate) -> Option<Variant> {
        let mut counts: Vec<(Variant, usize)> = vec![];
        for d in time::days_inclusive(from, to) {
            if let Some(v) = self.get(metric, d).filter(|s| s.value.is_some()).and_then(variant_of) {
                match counts.iter_mut().find(|(k, _)| *k == v) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((v, 1)),
                }
            }
        }
        let preferred = |v: &Variant| matches!(v, Variant::Hrv(HrvMethod::Rmssd) | Variant::Calories(CalorieScope::Total));
        counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(preferred(&a.0).cmp(&preferred(&b.0)))).map(|(v, _)| v)
    }

    pub fn latest(&self, metric: Metric, on_or_before: NaiveDate) -> Option<&DaySummary> {
        self.map
            .range((metric, NaiveDate::MIN)..=(metric, on_or_before))
            .rev()
            .map(|(_, s)| s)
            .find(|s| s.value.is_some())
    }
    pub fn first_day(&self) -> Option<NaiveDate> {
        self.map.keys().map(|(_, d)| *d).min()
    }
    pub fn last_day(&self) -> Option<NaiveDate> {
        self.map.keys().map(|(_, d)| *d).max()
    }
    pub fn has_metric(&self, metric: Metric) -> bool {
        self.map.keys().any(|(m, _)| *m == metric)
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dedupe::measurement_id;
    use crate::time::parse_tz;

    fn prov(origin: Origin) -> Provenance {
        Provenance {
            origin,
            status: if origin == Origin::BluetoothLive { DataStatus::Measured } else { DataStatus::Imported },
            source_name: if origin == Origin::BluetoothLive { "ble".into() } else { "file".into() },
            device: None,
            original_id: None,
            batch_id: None,
            ingested_at: 0,
            provisional: false,
            coverage: None,
            confidence: None,
            calc_version: None,
            demo: false,
        }
    }

    fn m(metric: Metric, kind: SampleKind, start: i64, end: Option<i64>, value: f64, origin: Origin) -> Measurement {
        let mut x = Measurement {
            id: String::new(),
            metric,
            kind,
            start,
            end,
            utc_offset_min: None,
            day: None,
            value,
            unit: metric.canonical_unit(),
            detail: None,
            prov: prov(origin),
            superseded_by: None,
        };
        x.id = measurement_id(&x);
        x
    }

    fn ctx() -> SummaryContext {
        SummaryContext { tz: parse_tz("UTC"), now: i64::MAX / 2, demo: false }
    }

    const D0: i64 = 1_790_000_000_000 - 1_790_000_000_000 % 86_400_000; // a UTC midnight

    #[test]
    fn cumulative_counter_is_differenced_not_summed() {
        let h = 3_600_000;
        let items = vec![
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 8 * h, None, 1000.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 9 * h, None, 1500.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 10 * h, None, 4000.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, Some(3000.0)); // not 6500
    }

    #[test]
    fn counter_reset_counts_from_zero() {
        let h = 3_600_000;
        let items = vec![
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 8 * h, None, 1000.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 9 * h, None, 2000.0, Origin::BluetoothLive),
            // device rebooted: counter restarts
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 10 * h, None, 300.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 11 * h, None, 800.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, Some(1000.0 + 300.0 + 500.0));
    }

    #[test]
    fn daily_reset_counter_counts_first_reading() {
        let h = 3_600_000;
        let mut a = m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 8 * h, None, 1200.0, Origin::BluetoothLive);
        a.detail = Some(Detail::Counter { resets_daily: true });
        let mut b = m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 9 * h, None, 1500.0, Origin::BluetoothLive);
        b.detail = Some(Detail::Counter { resets_daily: true });
        let s = summarize(&[a, b], &ctx());
        assert_eq!(s[0].value, Some(1500.0));
    }

    #[test]
    fn reset_after_midnight_stays_in_its_own_day() {
        // Lifetime counter on day 0, then a reboot reading the next morning:
        // the reset value belongs to day 1 and is not spread back into day 0.
        let h = 3_600_000;
        let items = vec![
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 20 * h, None, 1000.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 22 * h, None, 1600.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 32 * h, None, 400.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        let total: f64 = s.iter().filter_map(|x| x.value).sum();
        assert_eq!(s[0].value, Some(600.0));
        assert_eq!(total, 1000.0);
    }

    #[test]
    fn implausible_counter_jump_rejected() {
        let mins = 60_000;
        let items = vec![
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 10 * mins, None, 100.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 11 * mins, None, 90_000.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, None);
        match &s[0].extra {
            Some(SummaryExtra::Additive { rejected_jumps, .. }) => assert_eq!(*rejected_jumps, 1),
            _ => panic!(),
        }
    }

    #[test]
    fn overlapping_import_and_live_not_double_counted() {
        let h = 3_600_000;
        let mut items = vec![];
        // imported 1-hour intervals from 08:00 to 12:00 (4 × 500)
        for i in 8..12 {
            items.push(m(Metric::Steps, SampleKind::Interval, D0 + i * h, Some(D0 + (i + 1) * h), 500.0, Origin::HistoryImport));
        }
        // live counter covering 10:00–14:00 (overlaps 10–12)
        for (i, v) in [(10, 0.0), (11, 400.0), (12, 900.0), (13, 1500.0), (14, 2000.0)] {
            items.push(m(Metric::Steps, SampleKind::CumulativeCounter, D0 + i * h, None, v, Origin::BluetoothLive));
        }
        let s = summarize(&items, &ctx());
        // 2000 imported + live only for 12–14 (1500-900 + 2000-1500 = 1100)
        assert_eq!(s[0].value, Some(3100.0));
    }

    #[test]
    fn imported_daily_total_wins() {
        let h = 3_600_000;
        let mut total = m(Metric::Steps, SampleKind::DailyTotal, D0, Some(D0 + 24 * h), 9000.0, Origin::HistoryImport);
        total.day = Some(time::day_key(time::local_day_in(D0, &parse_tz("UTC"))));
        let items = vec![
            total,
            m(Metric::Steps, SampleKind::Interval, D0 + h, Some(D0 + 2 * h), 400.0, Origin::HistoryImport),
        ];
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, Some(9000.0));
    }

    #[test]
    fn missing_data_is_none_not_zero() {
        let idx = SummaryIndex::default();
        let d = time::local_day_in(D0, &parse_tz("UTC"));
        let series = idx.series(Metric::Steps, d, d + chrono::Duration::days(2));
        assert!(series.iter().all(|(_, v)| v.is_none()));
    }

    #[test]
    fn overnight_sleep_belongs_to_wake_day() {
        let tz = parse_tz("Europe/Paris");
        let (bed, _) = time::parse_rfc3339("2026-06-01T23:30:00+02:00").unwrap();
        let (wake, _) = time::parse_rfc3339("2026-06-02T07:00:00+02:00").unwrap();
        let mut s = m(Metric::Sleep, SampleKind::Session, bed, Some(wake), 420.0, Origin::HistoryImport);
        s.detail = Some(Detail::Sleep(SleepDetail { stages: vec![], stages_measured: false, asleep_minutes_derived: false, is_main: true }));
        let c = SummaryContext { tz, now: i64::MAX / 2, demo: false };
        let out = summarize(&[s], &c);
        assert_eq!(out[0].day, "2026-06-02");
    }

    #[test]
    fn counter_increment_across_midnight_is_shared() {
        let h = 3_600_000;
        let items = vec![
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 22 * h, None, 1000.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 23 * h, None, 1500.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 25 * h, None, 2500.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 26 * h, None, 2600.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        // 22–23: 500, 23–01 (1000) split evenly, 01–02: 100. Nothing lost, nothing doubled.
        assert_eq!(s[0].value, Some(1000.0));
        assert_eq!(s[1].value, Some(600.0));
    }

    #[test]
    fn coverage_counts_intervals_not_spans() {
        let h = 3_600_000;
        let mut items = vec![
            m(Metric::Steps, SampleKind::Interval, D0 + 8 * h, Some(D0 + 9 * h), 500.0, Origin::HistoryImport),
            m(Metric::Steps, SampleKind::Interval, D0 + 20 * h, Some(D0 + 21 * h), 500.0, Origin::HistoryImport),
        ];
        for (i, v) in [(10, 0.0), (12, 1000.0), (14, 2000.0)] {
            let mut c = m(Metric::Steps, SampleKind::CumulativeCounter, D0 + i * h, None, v, Origin::HistoryImport);
            c.prov.source_name = "other".into();
            c.id = measurement_id(&c);
            items.push(c);
        }
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, Some(3000.0));
        let cov = s[0].coverage.unwrap();
        assert!((cov - 6.0 / 24.0).abs() < 1e-9, "{cov}");
    }

    #[test]
    fn partially_overlapping_counter_increment_is_prorated() {
        let h = 3_600_000;
        let q = 15 * 60_000;
        let items = vec![
            m(Metric::Steps, SampleKind::Interval, D0 + 8 * h, Some(D0 + 17 * h), 9000.0, Origin::HistoryImport),
            // live counter 16:45 → 17:15 (+600): half overlaps history
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 17 * h - q, None, 0.0, Origin::BluetoothLive),
            m(Metric::Steps, SampleKind::CumulativeCounter, D0 + 17 * h + q, None, 600.0, Origin::BluetoothLive),
        ];
        let s = summarize(&items, &ctx());
        assert_eq!(s[0].value, Some(9300.0));
    }

    #[test]
    fn overlapping_sleep_sessions_are_not_naps() {
        let h = 3_600_000;
        let mut a = m(Metric::Sleep, SampleKind::Session, D0 - 2 * h, Some(D0 + 6 * h), 420.0, Origin::HistoryImport);
        a.detail = Some(Detail::Sleep(SleepDetail { stages: vec![], stages_measured: false, asleep_minutes_derived: false, is_main: true }));
        let mut b = a.clone();
        b.value = 400.0;
        b.id = measurement_id(&b);
        let mut nap = a.clone();
        nap.start = D0 + 14 * h;
        nap.end = Some(D0 + 15 * h);
        nap.value = 50.0;
        nap.id = measurement_id(&nap);
        let s = summarize(&[a, b, nap], &ctx());
        assert!(matches!(s[0].extra, Some(SummaryExtra::Sleep { naps: 1, .. })), "{:?}", s[0].extra);
    }

    #[test]
    fn consistent_series_keeps_one_hrv_method() {
        let d = 86_400_000;
        let mk = |day: i64, v: f64, method: HrvMethod| {
            let mut x = m(Metric::Hrv, SampleKind::Instant, D0 + day * d + 1000, None, v, Origin::HistoryImport);
            x.detail = Some(Detail::Hrv { method });
            x.id = measurement_id(&x);
            x
        };
        let items = vec![mk(0, 40.0, HrvMethod::Rmssd), mk(0, 80.0, HrvMethod::Sdnn), mk(1, 90.0, HrvMethod::Sdnn), mk(2, 44.0, HrvMethod::Rmssd)];
        let idx = SummaryIndex::from_vec(summarize(&items, &ctx()));
        let d0 = time::local_day_in(D0, &parse_tz("UTC"));
        let (series, variant, excluded) = idx.consistent_series(Metric::Hrv, d0, d0 + chrono::Duration::days(2));
        assert_eq!(variant, Some(Variant::Hrv(HrvMethod::Rmssd)));
        assert_eq!(excluded, 1);
        let vals: Vec<f64> = series.into_iter().filter_map(|(_, v)| v).collect();
        assert_eq!(vals, vec![40.0, 44.0]);
    }

    #[test]
    fn hrv_methods_not_combined() {
        let mut a = m(Metric::Hrv, SampleKind::Instant, D0 + 1000, None, 40.0, Origin::HistoryImport);
        a.detail = Some(Detail::Hrv { method: HrvMethod::Rmssd });
        let mut b = m(Metric::Hrv, SampleKind::Instant, D0 + 2000, None, 90.0, Origin::HistoryImport);
        b.detail = Some(Detail::Hrv { method: HrvMethod::Sdnn });
        let s = summarize(&[a, b], &ctx());
        assert_eq!(s[0].value, Some(40.0));
        assert!(matches!(s[0].extra, Some(SummaryExtra::Hrv { excluded_other_methods: 1, .. })));
    }

    #[test]
    fn superseded_records_are_ignored() {
        let mut a = m(Metric::RestingHeartRate, SampleKind::Instant, D0 + 1000, None, 70.0, Origin::HistoryImport);
        a.superseded_by = Some("x".into());
        let b = m(Metric::RestingHeartRate, SampleKind::Instant, D0 + 1000, None, 58.0, Origin::HistoryImport);
        let s = summarize(&[a, b], &ctx());
        assert_eq!(s[0].value, Some(58.0));
    }
}
