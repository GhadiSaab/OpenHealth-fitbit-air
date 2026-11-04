//! Time and local-day handling.
//!
//! Rules (documented in docs/ARCHITECTURE.md):
//! * All instants are stored as UTC milliseconds.
//! * A measurement's local day uses the offset recorded with it when the
//!   source provided one; otherwise the user's home time zone.
//! * Day boundaries are computed in the home time zone, so days around
//!   daylight-saving transitions are 23 or 25 hours long.
//! * A sleep session belongs to the local day on which it *ends* (the
//!   morning you wake up), so a night from 23:30 to 07:10 is "Tuesday's
//!   sleep" if you woke on Tuesday.

use chrono::{DateTime, Datelike, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

pub const MINUTE_MS: i64 = 60_000;
pub const HOUR_MS: i64 = 3_600_000;
pub const DAY_MS: i64 = 86_400_000;

/// Parses an IANA zone name, falling back to UTC.
pub fn parse_tz(name: &str) -> Tz {
    name.parse::<Tz>().unwrap_or(Tz::UTC)
}

pub fn day_key(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn parse_day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

/// The UTC instant of local midnight at the start of `day` in `tz`.
///
/// If midnight does not exist (a DST gap at 00:00, as in some zones), the
/// first valid instant after it is used.
pub fn day_start(day: NaiveDate, tz: &Tz) -> i64 {
    let mut t = day.and_time(NaiveTime::MIN);
    for _ in 0..4 {
        match tz.from_local_datetime(&t) {
            LocalResult::Single(dt) => return dt.timestamp_millis(),
            LocalResult::Ambiguous(a, _) => return a.timestamp_millis(),
            LocalResult::None => t += Duration::minutes(30),
        }
    }
    Utc.from_utc_datetime(&day.and_time(NaiveTime::MIN)).timestamp_millis()
}

/// `[start, end)` of a local day in UTC ms. Not always 24 h long.
pub fn day_bounds(day: NaiveDate, tz: &Tz) -> (i64, i64) {
    (day_start(day, tz), day_start(day.succ_opt().unwrap_or(day), tz))
}

/// Local day of an instant in `tz`.
pub fn local_day_in(ts: i64, tz: &Tz) -> NaiveDate {
    match DateTime::<Utc>::from_timestamp_millis(ts) {
        Some(dt) => dt.with_timezone(tz).date_naive(),
        None => NaiveDate::default(),
    }
}

/// Local day of an instant, preferring the offset recorded with it.
pub fn local_day(ts: i64, offset_min: Option<i32>, tz: &Tz) -> NaiveDate {
    match offset_min.and_then(|m| FixedOffset::east_opt(m * 60)) {
        Some(off) => match DateTime::<Utc>::from_timestamp_millis(ts) {
            Some(dt) => dt.with_timezone(&off).date_naive(),
            None => NaiveDate::default(),
        },
        None => local_day_in(ts, tz),
    }
}

/// Minutes since local midnight for an instant in `tz` (0..1440).
pub fn minute_of_day(ts: i64, tz: &Tz) -> i32 {
    match DateTime::<Utc>::from_timestamp_millis(ts) {
        Some(dt) => {
            let l = dt.with_timezone(tz);
            (l.time().signed_duration_since(NaiveTime::MIN).num_minutes()) as i32
        }
        None => 0,
    }
}

/// Formats an instant in `tz` with a chrono format string.
pub fn format_ts(ts: i64, tz: &Tz, fmt: &str) -> String {
    match DateTime::<Utc>::from_timestamp_millis(ts) {
        Some(dt) => dt.with_timezone(tz).format(fmt).to_string(),
        None => String::from("—"),
    }
}

/// Current UTC offset (minutes) of `tz` at instant `ts`.
pub fn offset_minutes_at(ts: i64, tz: &Tz) -> i32 {
    use chrono::Offset;
    match DateTime::<Utc>::from_timestamp_millis(ts) {
        Some(dt) => dt.with_timezone(tz).offset().fix().local_minus_utc() / 60,
        None => 0,
    }
}

/// Parses an RFC 3339 timestamp (with offset) into (UTC ms, offset minutes).
pub fn parse_rfc3339(s: &str) -> Option<(i64, i32)> {
    let dt = DateTime::parse_from_rfc3339(s.trim()).ok()?;
    Some((dt.timestamp_millis(), dt.offset().local_minus_utc() / 60))
}

/// Parses a timestamp that may lack an offset. Offset-less local times are
/// resolved in `tz`; ambiguous local times (DST fall-back) resolve to the
/// earlier instant and are reported via the returned flag.
pub fn parse_timestamp(s: &str, tz: &Tz) -> Option<ParsedTime> {
    let s = s.trim();
    if let Some((ms, off)) = parse_rfc3339(s) {
        return Some(ParsedTime { utc_ms: ms, offset_min: Some(off), ambiguous: false });
    }
    if let Ok(ms) = s.parse::<i64>() {
        // Plain integers are epoch milliseconds (or seconds if clearly too small).
        let ms = if ms.abs() < 100_000_000_000 { ms * 1000 } else { ms };
        return Some(ParsedTime { utc_ms: ms, offset_min: None, ambiguous: false });
    }
    let formats = ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M", "%Y-%m-%d %H:%M"];
    for f in formats {
        if let Ok(naive) = NaiveDateTime::parse_from_str(s, f) {
            return match tz.from_local_datetime(&naive) {
                LocalResult::Single(dt) => Some(ParsedTime {
                    utc_ms: dt.timestamp_millis(),
                    offset_min: None,
                    ambiguous: false,
                }),
                LocalResult::Ambiguous(a, _) => Some(ParsedTime {
                    utc_ms: a.timestamp_millis(),
                    offset_min: None,
                    ambiguous: true,
                }),
                LocalResult::None => None,
            };
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedTime {
    pub utc_ms: i64,
    pub offset_min: Option<i32>,
    pub ambiguous: bool,
}

/// Monday of the ISO week containing `d`.
pub fn week_start(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap_or(d)
}

pub fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    let (y, m0) = (total.div_euclid(12), total.rem_euclid(12));
    let mut day = d.day();
    loop {
        if let Some(r) = NaiveDate::from_ymd_opt(y, m0 as u32 + 1, day) {
            return r;
        }
        day -= 1;
    }
}

/// Iterates days in `[from, to]` inclusive.
pub fn days_inclusive(from: NaiveDate, to: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    let n = (to - from).num_days().max(-1) + 1;
    (0..n).map(move |i| from + Duration::days(i))
}

/// Formats a duration in minutes as "7 h 32 min".
pub fn fmt_minutes(min: f64) -> String {
    let m = min.round() as i64;
    if m < 60 {
        format!("{m} min")
    } else {
        format!("{} h {:02} min", m / 60, m % 60)
    }
}

/// Human description of how long ago `ts` was, relative to `now`.
pub fn fmt_ago(ts: i64, now: i64) -> String {
    let d = (now - ts).max(0) / MINUTE_MS;
    match d {
        0 => "just now".into(),
        1..=59 => format!("{d} min ago"),
        60..=1439 => {
            let h = d / 60;
            let m = d % 60;
            if m == 0 { format!("{h} h ago") } else { format!("{h} h {m} min ago") }
        }
        _ => {
            let days = d / 1440;
            if days == 1 { "yesterday".into() } else { format!("{days} days ago") }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dst_spring_forward_day_is_23_hours() {
        let tz = parse_tz("Europe/Berlin");
        let d = NaiveDate::from_ymd_opt(2026, 3, 29).unwrap();
        let (s, e) = day_bounds(d, &tz);
        assert_eq!(e - s, 23 * HOUR_MS);
    }

    #[test]
    fn dst_fall_back_day_is_25_hours() {
        let tz = parse_tz("America/New_York");
        let d = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let (s, e) = day_bounds(d, &tz);
        assert_eq!(e - s, 25 * HOUR_MS);
    }

    #[test]
    fn recorded_offset_wins_over_home_zone() {
        // 2026-06-01 23:30 in Tokyo (+09:00) is 14:30 UTC; in London it's 15:30 on the same day,
        // but 2026-06-01 01:30 Tokyo time is the previous day in London.
        let tz = parse_tz("Europe/London");
        let (ts, off) = parse_rfc3339("2026-06-01T01:30:00+09:00").unwrap();
        assert_eq!(local_day(ts, Some(off), &tz), NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert_eq!(local_day(ts, None, &tz), NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
    }

    #[test]
    fn ambiguous_local_time_is_flagged() {
        let tz = parse_tz("America/New_York");
        let p = parse_timestamp("2026-11-01 01:30:00", &tz).unwrap();
        assert!(p.ambiguous);
        let p2 = parse_timestamp("2026-11-01 03:30:00", &tz).unwrap();
        assert!(!p2.ambiguous);
    }

    #[test]
    fn nonexistent_local_time_is_rejected() {
        let tz = parse_tz("America/New_York");
        assert!(parse_timestamp("2026-03-08 02:30:00", &tz).is_none());
    }

    #[test]
    fn months_clamp() {
        let d = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(add_months(d, 1), NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
        assert_eq!(add_months(d, -2), NaiveDate::from_ymd_opt(2025, 11, 30).unwrap());
    }
}
