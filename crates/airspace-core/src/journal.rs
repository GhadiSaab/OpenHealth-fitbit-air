//! Journal entries, tags and cautious tag–metric associations.

use crate::aggregate::SummaryIndex;
use crate::model::Metric;
use crate::time;
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

pub const SUGGESTED_TAGS: [&str; 8] = ["exercise", "illness", "caffeine", "alcohol", "travel", "stress", "late meal", "rest day"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JournalEntry {
    pub id: String,
    /// Local day YYYY-MM-DD.
    pub day: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub demo: bool,
}

impl JournalEntry {
    pub fn new(day: NaiveDate, now: i64) -> Self {
        JournalEntry {
            id: format!("j-{}", time::day_key(day)),
            day: time::day_key(day),
            note: String::new(),
            tags: vec![],
            created_at: now,
            updated_at: now,
            demo: false,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.note.trim().is_empty() && self.tags.is_empty()
    }
}

/// Normalizes a user-entered tag.
pub fn normalize_tag(s: &str) -> Option<String> {
    let t: String = s.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() || t.len() > 32 { None } else { Some(t) }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Association {
    pub tag: String,
    pub metric: Metric,
    /// 0 = same day, 1 = the following day (e.g. alcohol → next night's sleep).
    pub lag_days: i64,
    pub tagged_n: usize,
    pub other_n: usize,
    pub tagged_mean: f64,
    pub other_mean: f64,
    pub window: (NaiveDate, NaiveDate),
}

impl Association {
    pub fn diff(&self) -> f64 {
        self.tagged_mean - self.other_mean
    }
}

pub const MIN_TAGGED_DAYS: usize = 3;
pub const MIN_OTHER_DAYS: usize = 5;

/// Compares a metric on days with a tag against days without it, within
/// `[from, to]`. Returns `None` if there isn't enough data on both sides.
///
/// This describes an association in the user's own records. It is not, and
/// must never be presented as, evidence of cause.
pub fn associate(
    entries: &[JournalEntry],
    summaries: &SummaryIndex,
    tag: &str,
    metric: Metric,
    lag_days: i64,
    from: NaiveDate,
    to: NaiveDate,
) -> Option<Association> {
    let tagged_days: std::collections::HashSet<NaiveDate> = entries
        .iter()
        .filter(|e| e.tags.iter().any(|t| t == tag))
        .filter_map(|e| time::parse_day(&e.day))
        .collect();
    let (mut a, mut b) = (vec![], vec![]);
    for d in time::days_inclusive(from, to) {
        let Some(v) = summaries.value(metric, d + Duration::days(lag_days)) else { continue };
        if tagged_days.contains(&d) { a.push(v) } else { b.push(v) }
    }
    if a.len() < MIN_TAGGED_DAYS || b.len() < MIN_OTHER_DAYS {
        return None;
    }
    Some(Association {
        tag: tag.into(),
        metric,
        lag_days,
        tagged_n: a.len(),
        other_n: b.len(),
        tagged_mean: a.iter().sum::<f64>() / a.len() as f64,
        other_mean: b.iter().sum::<f64>() / b.len() as f64,
        window: (from, to),
    })
}

/// Metrics worth comparing against tags, with the lag that makes sense.
pub const ASSOCIATION_METRICS: [(Metric, i64); 5] = [
    (Metric::Sleep, 1),
    (Metric::RestingHeartRate, 1),
    (Metric::Hrv, 1),
    (Metric::Steps, 0),
    (Metric::SkinTemperature, 1),
];

/// All tags used in the journal, most frequent first.
pub fn tag_counts(entries: &[JournalEntry]) -> Vec<(String, usize)> {
    let mut m: std::collections::BTreeMap<String, usize> = Default::default();
    for e in entries {
        for t in &e.tags {
            *m.entry(t.clone()).or_default() += 1;
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v
}
