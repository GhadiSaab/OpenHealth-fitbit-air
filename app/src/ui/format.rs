//! Display helpers shared by screens.

use airspace_core::Metric;
use airspace_core::settings::{Settings, display};
use chrono::{Datelike, Duration, NaiveDate};

pub fn day_title(d: NaiveDate, today: NaiveDate) -> String {
    if d == today {
        "Today".into()
    } else if d == today - Duration::days(1) {
        "Yesterday".into()
    } else if d.year() == today.year() {
        d.format("%A, %-d %B").to_string()
    } else {
        d.format("%-d %B %Y").to_string()
    }
}

pub fn day_short(d: NaiveDate) -> String {
    d.format("%a %-d %b").to_string()
}

pub fn value(metric: Metric, v: f64, s: &Settings) -> String {
    if metric == Metric::Sleep {
        return airspace_core::time::fmt_minutes(v);
    }
    display(metric, v, s).joined()
}

/// Signed difference with units, e.g. "+812 steps", "−1.2 bpm".
pub fn delta(metric: Metric, d: f64, s: &Settings) -> String {
    let txt = if metric == Metric::Sleep { airspace_core::time::fmt_minutes(d.abs()) } else { display(metric, d.abs(), s).joined() };
    let txt = txt.trim_start_matches(['+', '−', '±']).to_string();
    // A change that rounds to zero is shown as no change, never "−0".
    let zero = txt.trim_start_matches(['0', '.', ',']).chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(true);
    if zero || d.abs() < 1e-9 { format!("±{txt}") } else if d > 0.0 { format!("+{txt}") } else { format!("−{txt}") }
}

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{}\u{a0}{}", count(n), if n == 1 { one } else { many })
}

/// "46,393"
pub fn count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
