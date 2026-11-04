//! The shared intermediate record and its validation.

use super::{ImportContext, Issue, Severity};
use crate::dedupe::measurement_id;
use crate::model::*;
use crate::time::{self, MINUTE_MS, parse_timestamp};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

/// One record of the Airspace import schema. Every field except `metric`
/// and `value` (or `start`/`end` for sleep) is optional.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RawRecord {
    pub metric: String,
    #[serde(default)]
    pub kind: Option<String>,
    /// Instant timestamp (alias for `start` on instant/counter records).
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    /// Local day for daily totals (YYYY-MM-DD).
    #[serde(default)]
    pub day: Option<String>,
    #[serde(default)]
    pub value: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    /// Identifier in the original source.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub confidence: Option<u8>,
    #[serde(default)]
    pub provisional: Option<bool>,
    // Metric-specific fields
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub zone: Option<String>,
    #[serde(default)]
    pub lower_bpm: Option<f64>,
    #[serde(default)]
    pub upper_bpm: Option<f64>,
    #[serde(default)]
    pub zone_origin: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub resets_daily: Option<bool>,
    #[serde(default)]
    pub asleep_minutes: Option<f64>,
    #[serde(default)]
    pub main: Option<bool>,
    #[serde(default)]
    pub stages: Option<Vec<RawStage>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RawStage {
    pub stage: String,
    pub start: String,
    pub end: String,
}

/// Source description shared by all records of one file.
#[derive(Clone, Debug)]
pub struct SourceInfo {
    pub name: String,
    pub device: Option<String>,
    pub adapter: &'static str,
    pub tz: Tz,
}

fn kind_for(r: &RawRecord, metric: Metric) -> Result<SampleKind, String> {
    if let Some(k) = &r.kind {
        return Ok(match k.trim().to_ascii_lowercase().as_str() {
            "instant" | "sample" => SampleKind::Instant,
            "interval" => SampleKind::Interval,
            "cumulative_counter" | "counter" | "cumulative" => SampleKind::CumulativeCounter,
            "daily_total" | "daily" | "day" => SampleKind::DailyTotal,
            "session" => SampleKind::Session,
            other => return Err(format!("Unknown kind \"{other}\".")),
        });
    }
    Ok(if metric == Metric::Sleep {
        SampleKind::Session
    } else if r.day.is_some() && r.start.is_none() && r.time.is_none() {
        SampleKind::DailyTotal
    } else if r.start.is_some() && r.end.is_some() {
        SampleKind::Interval
    } else {
        SampleKind::Instant
    })
}

/// Converts a value in `unit` to the metric's canonical unit.
fn convert_unit(metric: Metric, value: f64, unit: Option<&str>) -> Result<f64, String> {
    let Some(u) = unit.map(|u| u.trim().to_ascii_lowercase()).filter(|u| !u.is_empty()) else {
        return Ok(value);
    };
    let canonical = metric.canonical_unit();
    if Unit::parse(&u) == Some(canonical) {
        return Ok(value);
    }
    let converted = match (metric, u.as_str()) {
        (Metric::Distance, "km") => Some(value * 1000.0),
        (Metric::Distance, "mi" | "miles") => Some(value * 1609.344),
        (Metric::Distance, "ft" | "feet") => Some(value * 0.3048),
        (Metric::SkinTemperature, "delta_f" | "°f delta" | "f_delta") => Some(value * 5.0 / 9.0),
        (Metric::Sleep, "h" | "hours") => Some(value * 60.0),
        (Metric::Sleep | Metric::HeartRateZones, "s" | "seconds") => Some(value / 60.0),
        (Metric::Calories, "kj") => Some(value / 4.184),
        (Metric::SkinTemperature, "c" | "°c" | "celsius" | "f" | "°f" | "fahrenheit") => {
            return Err("Skin temperature must be a deviation from baseline (unit \"delta_c\" or \"delta_f\"), not an absolute temperature.".into());
        }
        _ => None,
    };
    converted.ok_or_else(|| format!("Unit \"{u}\" is not valid for {} (expected \"{}\").", metric.key(), canonical.key()))
}

/// Plausible range per metric. Values outside are rejected.
fn plausible(metric: Metric) -> (f64, f64) {
    match metric {
        Metric::HeartRate => (25.0, 250.0),
        Metric::RestingHeartRate => (25.0, 150.0),
        Metric::Spo2 => (50.0, 100.0),
        Metric::Hrv => (1.0, 400.0),
        Metric::RespiratoryRate => (3.0, 60.0),
        Metric::SkinTemperature => (-6.0, 6.0),
        Metric::Steps => (0.0, 150_000.0),
        Metric::Distance => (0.0, 300_000.0),
        Metric::Calories => (0.0, 20_000.0),
        Metric::Sleep => (0.0, 24.0 * 60.0),
        Metric::HeartRateZones => (0.0, 24.0 * 60.0),
    }
}

fn ts(s: &Option<String>, field: &str, tz: &Tz, warn: &mut Vec<String>) -> Result<Option<(i64, Option<i32>)>, String> {
    match s {
        None => Ok(None),
        Some(v) => {
            let p = parse_timestamp(v, tz).ok_or_else(|| format!("\"{field}\" is not a valid timestamp (\"{v}\")."))?;
            if p.ambiguous {
                warn.push(format!("\"{field}\" {v} is ambiguous because of a daylight-saving change; the earlier time was used. Add an offset to be exact."));
            }
            Ok(Some((p.utc_ms, p.offset_min)))
        }
    }
}

/// Validates and converts one record. Returns the measurement and warnings,
/// or an error message (the record is then skipped).
pub fn to_measurement(r: &RawRecord, src: &SourceInfo, ctx: &ImportContext) -> Result<(Measurement, Vec<String>), String> {
    let mut warn = vec![];
    let metric = Metric::from_key(r.metric.trim()).ok_or_else(|| format!("Unknown metric \"{}\".", r.metric))?;
    let kind = kind_for(r, metric)?;
    let tz = &src.tz;
    let start = ts(&r.start, "start", tz, &mut warn)?.or(ts(&r.time, "time", tz, &mut warn)?);
    let end = ts(&r.end, "end", tz, &mut warn)?;
    let (start_ms, offset, end_ms, day);
    match kind {
        SampleKind::DailyTotal => {
            let d = r.day.as_deref().and_then(time::parse_day).ok_or("Daily totals need \"day\" as YYYY-MM-DD.")?;
            let (a, b) = time::day_bounds(d, tz);
            start_ms = a;
            offset = None;
            end_ms = Some(b);
            day = Some(time::day_key(d));
        }
        SampleKind::Interval | SampleKind::Session => {
            let (a, o) = start.ok_or("Intervals and sessions need \"start\".")?;
            let (b, _) = end.ok_or("Intervals and sessions need \"end\".")?;
            if b <= a {
                return Err("\"end\" must be after \"start\".".into());
            }
            if b - a > 24 * 60 * MINUTE_MS {
                return Err("Interval is longer than 24 hours.".into());
            }
            start_ms = a;
            offset = o;
            end_ms = Some(b);
            day = None;
        }
        SampleKind::Instant | SampleKind::CumulativeCounter => {
            let (a, o) = start.ok_or("Readings need \"time\".")?;
            start_ms = a;
            offset = o;
            end_ms = None;
            day = None;
        }
    }
    if start_ms > ctx.now + 24 * 60 * MINUTE_MS {
        return Err("Timestamp is in the future.".into());
    }
    if start_ms < 946_684_800_000 {
        return Err("Timestamp is before the year 2000.".into());
    }

    let mut detail = None;
    let mut value = r.value;
    if metric == Metric::Sleep {
        if kind != SampleKind::Session {
            return Err("Sleep records must be sessions with start and end.".into());
        }
        let (a, b) = (start_ms, end_ms.unwrap());
        let mut stages = vec![];
        for (i, st) in r.stages.iter().flatten().enumerate() {
            let stage = SleepStage::parse(&st.stage).ok_or_else(|| format!("Stage {} has unknown type \"{}\".", i + 1, st.stage))?;
            let s0 = parse_timestamp(&st.start, tz).ok_or_else(|| format!("Stage {} start is invalid.", i + 1))?.utc_ms;
            let s1 = parse_timestamp(&st.end, tz).ok_or_else(|| format!("Stage {} end is invalid.", i + 1))?.utc_ms;
            if s1 <= s0 {
                return Err(format!("Stage {} ends before it starts.", i + 1));
            }
            if s0 < a - MINUTE_MS || s1 > b + MINUTE_MS {
                warn.push(format!("Stage {} lies partly outside the session and was clipped.", i + 1));
            }
            stages.push(StageInterval { stage, start: s0.max(a), end: s1.min(b) });
        }
        stages.sort_by_key(|s| s.start);
        if stages.windows(2).any(|w| w[1].start < w[0].end) {
            warn.push("Some sleep stages overlap; stage totals may be overstated.".into());
        }
        let stages_measured = !stages.is_empty();
        let derived = r.asleep_minutes.is_none() && value.is_none();
        let asleep = match (r.asleep_minutes.or(value), stages_measured) {
            (Some(v), _) => v,
            (None, true) => stages.iter().filter(|s| s.stage.is_asleep()).map(|s| (s.end - s.start) as f64 / MINUTE_MS as f64).sum(),
            (None, false) => {
                warn.push("No asleep minutes or stages; time in bed is used as the sleep duration.".into());
                (b - a) as f64 / MINUTE_MS as f64
            }
        };
        value = Some(asleep);
        detail = Some(Detail::Sleep(SleepDetail {
            stages,
            stages_measured,
            asleep_minutes_derived: derived,
            is_main: r.main.unwrap_or(true),
        }));
    }

    let value = value.ok_or("Missing \"value\".")?;
    if !value.is_finite() {
        return Err("Value is not a finite number.".into());
    }
    let mut value = if metric == Metric::Sleep { value } else { convert_unit(metric, value, r.unit.as_deref())? };
    let (lo, hi) = plausible(metric);
    if value < lo || value > hi {
        return Err(format!("Value {value} is outside the plausible range for {} ({lo}–{hi}).", metric.key()));
    }
    if metric == Metric::Steps && kind == SampleKind::Interval {
        let mins = (end_ms.unwrap() - start_ms) as f64 / MINUTE_MS as f64;
        if value / mins.max(1.0) > 300.0 {
            warn.push("More than 300 steps per minute in this interval.".into());
        }
    }

    match metric {
        Metric::Hrv => {
            let method = r.method.as_deref().map(HrvMethod::parse).unwrap_or(HrvMethod::Unknown);
            if method == HrvMethod::Unknown {
                warn.push("HRV method not given; it will not be combined with RMSSD values.".into());
            }
            detail = Some(Detail::Hrv { method });
        }
        Metric::Calories => {
            let scope = match r.scope.as_deref().map(|s| s.trim().to_ascii_lowercase()) {
                Some(s) if s == "total" => CalorieScope::Total,
                Some(s) if s == "active" => CalorieScope::Active,
                _ => CalorieScope::Unspecified,
            };
            detail = Some(Detail::Calories { scope });
        }
        Metric::HeartRateZones => {
            let zone = r.zone.as_deref().and_then(HrZone::parse).ok_or("Zone records need \"zone\" (below_zones, fat_burn, cardio or peak).")?;
            let origin = match r.zone_origin.as_deref() {
                Some("calculated") => ZoneOrigin::Calculated,
                _ => ZoneOrigin::Imported,
            };
            detail = Some(Detail::Zone { zone, lower_bpm: r.lower_bpm, upper_bpm: r.upper_bpm, origin });
        }
        Metric::Spo2 => {
            if let (Some(mn), Some(mx)) = (r.min, r.max) {
                if mn > value || mx < value {
                    warn.push("SpO₂ range does not contain the average.".into());
                }
                detail = Some(Detail::Range { min: mn, max: mx });
            }
        }
        Metric::RespiratoryRate => {
            let stage = r.stage.as_deref().and_then(SleepStage::parse);
            detail = Some(Detail::Respiratory { stage });
        }
        Metric::Steps | Metric::Distance if kind == SampleKind::CumulativeCounter => {
            detail = Some(Detail::Counter { resets_daily: r.resets_daily.unwrap_or(false) });
        }
        _ => {}
    }
    if metric == Metric::Distance {
        value = (value * 100.0).round() / 100.0;
    }

    let mut m = Measurement {
        id: String::new(),
        metric,
        kind,
        start: start_ms,
        end: end_ms,
        utc_offset_min: offset,
        day,
        value,
        unit: metric.canonical_unit(),
        detail,
        prov: Provenance {
            origin: Origin::HistoryImport,
            status: DataStatus::Imported,
            source_name: src.name.clone(),
            device: src.device.clone(),
            original_id: r.id.clone(),
            batch_id: Some(ctx.batch_id.clone()),
            ingested_at: ctx.now,
            provisional: r.provisional.unwrap_or(false),
            coverage: None,
            confidence: r.confidence,
            calc_version: None,
            demo: false,
        },
        superseded_by: None,
    };
    m.id = measurement_id(&m);
    Ok((m, warn))
}

/// Converts many records, collecting issues with 1-based line numbers.
pub fn convert_all(raws: &[(usize, RawRecord)], src: &SourceInfo, ctx: &ImportContext, out: &mut super::ParsedImport) {
    for (line, r) in raws {
        match to_measurement(r, src, ctx) {
            Ok((m, warns)) => {
                for w in warns {
                    out.issues.push(Issue { line: *line, severity: Severity::Warning, message: w });
                }
                out.records.push(m);
            }
            Err(e) => out.issues.push(Issue { line: *line, severity: Severity::Error, message: e }),
        }
    }
}
