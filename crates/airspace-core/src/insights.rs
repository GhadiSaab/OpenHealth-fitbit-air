//! Local, explainable insights. No AI, no network.
//!
//! Every insight states its comparison window and how many days in it had
//! data. Insights are only produced when coverage is sufficient. There is
//! no readiness, sleep or recovery *score*: those would imply a validated
//! model Airspace does not have.

use crate::aggregate::{SummaryExtra, SummaryIndex};
use crate::catalog;
use crate::model::Metric;
use crate::series::std_dev;
use crate::settings::{Settings, display};
use crate::time;
use chrono::{Duration, NaiveDate};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Window {
    pub from: String,
    pub to: String,
    pub days_with_data: usize,
    pub days: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Insight {
    pub id: String,
    pub metric: Metric,
    pub title: String,
    pub body: String,
    pub window: Window,
    #[serde(default)]
    pub baseline: Option<Window>,
    /// Direction of change, for an unobtrusive indicator. Never "good"/"bad".
    pub direction: Direction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Up,
    Down,
    Steady,
    Info,
}

/// A comparison between two equally long periods.
#[derive(Clone, Debug, PartialEq)]
pub struct PeriodComparison {
    pub current: Option<f64>,
    pub previous: Option<f64>,
    pub current_days: usize,
    pub previous_days: usize,
    pub days: usize,
}

impl PeriodComparison {
    pub fn delta(&self) -> Option<f64> {
        Some(self.current? - self.previous?)
    }
    /// Comparison is reported only if both periods have at least half their
    /// days with data.
    pub fn reliable(&self) -> bool {
        self.current_days * 2 >= self.days && self.previous_days * 2 >= self.days && self.current_days > 0
    }
}

fn mean_over(idx: &SummaryIndex, metric: Metric, from: NaiveDate, to: NaiveDate) -> (Option<f64>, usize, Vec<f64>) {
    let vals: Vec<f64> = idx.series(metric, from, to).into_iter().filter_map(|(_, v)| v).collect();
    let n = vals.len();
    (if n == 0 { None } else { Some(vals.iter().sum::<f64>() / n as f64) }, n, vals)
}

/// Compares the mean daily value of `[from, to]` with the equally long period before it.
pub fn compare_periods(idx: &SummaryIndex, metric: Metric, from: NaiveDate, to: NaiveDate) -> PeriodComparison {
    let len = (to - from).num_days() + 1;
    let (c, cn, _) = mean_over(idx, metric, from, to);
    let (p, pn, _) = mean_over(idx, metric, from - Duration::days(len), from - Duration::days(1));
    PeriodComparison { current: c, previous: p, current_days: cn, previous_days: pn, days: len as usize }
}

fn window(from: NaiveDate, to: NaiveDate, n: usize) -> Window {
    Window { from: time::day_key(from), to: time::day_key(to), days_with_data: n, days: ((to - from).num_days() + 1) as usize }
}

fn dir(delta: f64, steady: f64) -> Direction {
    if delta > steady {
        Direction::Up
    } else if delta < -steady {
        Direction::Down
    } else {
        Direction::Steady
    }
}

fn fmt(metric: Metric, v: f64, s: &Settings) -> String {
    display(metric, v, s).joined()
}

/// Circular mean of times of day (minutes), handling the midnight wrap.
pub fn circular_mean_minutes(mins: &[f64]) -> Option<f64> {
    if mins.is_empty() {
        return None;
    }
    let (mut x, mut y) = (0.0, 0.0);
    for m in mins {
        let a = m / 1440.0 * std::f64::consts::TAU;
        x += a.cos();
        y += a.sin();
    }
    let a = y.atan2(x);
    Some((a / std::f64::consts::TAU * 1440.0).rem_euclid(1440.0))
}

/// Deviation of `m` from `center` in minutes, wrapped to [-720, 720).
pub fn wrap_diff(m: f64, center: f64) -> f64 {
    (m - center + 720.0).rem_euclid(1440.0) - 720.0
}

pub fn fmt_clock(min: f64) -> String {
    let m = min.round() as i64 % 1440;
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Generates insights for the period ending on `today`.
pub fn generate(idx: &SummaryIndex, today: NaiveDate, tz: &Tz, s: &Settings) -> Vec<Insight> {
    let mut out = vec![];
    let w7 = (today - Duration::days(6), today);
    let p7 = (today - Duration::days(13), today - Duration::days(7));

    // Week-over-week comparisons for activity and sleep duration.
    for metric in [Metric::Steps, Metric::Sleep] {
        // Today's step count is still growing: compare complete days only.
        let (w7, p7) = if metric.is_additive() { ((w7.0 - Duration::days(1), w7.1 - Duration::days(1)), (p7.0 - Duration::days(1), p7.1 - Duration::days(1))) } else { (w7, p7) };
        let c = compare_periods(idx, metric, w7.0, w7.1);
        if c.current_days >= 5 && c.previous_days >= 5 {
            let (cur, prev) = (c.current.unwrap(), c.previous.unwrap());
            let d = cur - prev;
            let steady = if metric == Metric::Steps { prev * 0.05 } else { 10.0 };
            let name = catalog::info(metric).short.to_lowercase();
            let body = match dir(d, steady) {
                Direction::Steady => format!(
                    "Your daily {name} averaged {} over the last 7 days, about the same as the 7 days before ({}).",
                    fmt(metric, cur, s),
                    fmt(metric, prev, s)
                ),
                _ => format!(
                    "Your daily {name} averaged {} over the last 7 days, {} the previous 7 days ({}).",
                    fmt(metric, cur, s),
                    if d > 0.0 { "up from" } else { "down from" },
                    fmt(metric, prev, s)
                ),
            };
            out.push(Insight {
                id: format!("wow-{}", metric.key()),
                metric,
                title: format!("{} this week", catalog::info(metric).title),
                body,
                window: window(w7.0, w7.1, c.current_days),
                baseline: Some(window(p7.0, p7.1, c.previous_days)),
                direction: dir(d, steady),
            });
        }
    }

    // Resting heart rate and HRV relative to a 28-day personal baseline.
    for (metric, steady) in [(Metric::RestingHeartRate, 1.5), (Metric::Hrv, 3.0)] {
        let base = (today - Duration::days(34), today - Duration::days(7));
        let (cur, cn, _) = mean_over(idx, metric, w7.0, w7.1);
        let (b, bn, _) = mean_over(idx, metric, base.0, base.1);
        if cn >= 4 && bn >= 14 {
            let (cur, b) = (cur.unwrap(), b.unwrap());
            let d = cur - b;
            let method = if metric == Metric::Hrv {
                match idx.latest(metric, today).and_then(|x| x.extra.clone()) {
                    Some(SummaryExtra::Hrv { method, .. }) => format!(" ({})", method.label()),
                    _ => String::new(),
                }
            } else {
                String::new()
            };
            let rel = match dir(d, steady) {
                Direction::Steady => "close to".to_string(),
                Direction::Up => format!("{} above", fmt(metric, d.abs(), s)),
                _ => format!("{} below", fmt(metric, d.abs(), s)),
            };
            out.push(Insight {
                id: format!("baseline-{}", metric.key()),
                metric,
                title: format!("{} vs your baseline", catalog::info(metric).title),
                body: format!(
                    "Your 7-day average{method} is {}, {rel} your baseline of {} from the preceding weeks.",
                    fmt(metric, cur, s),
                    fmt(metric, b, s)
                ),
                window: window(w7.0, w7.1, cn),
                baseline: Some(window(base.0, base.1, bn)),
                direction: dir(d, steady),
            });
        }
    }

    // Sleep timing consistency over 14 nights.
    let w14 = (today - Duration::days(13), today);
    let mut beds = vec![];
    let mut wakes = vec![];
    for d in time::days_inclusive(w14.0, w14.1) {
        if let Some(sm) = idx.get(Metric::Sleep, d) {
            if let Some(SummaryExtra::Sleep { bed, wake, .. }) = &sm.extra {
                beds.push(time::minute_of_day(*bed, tz) as f64);
                wakes.push(time::minute_of_day(*wake, tz) as f64);
            }
        }
    }
    if beds.len() >= 7 {
        let cb = circular_mean_minutes(&beds).unwrap();
        let cw = circular_mean_minutes(&wakes).unwrap();
        let sd_b = std_dev(&beds.iter().map(|m| wrap_diff(*m, cb)).collect::<Vec<_>>()).unwrap_or(0.0);
        let sd_w = std_dev(&wakes.iter().map(|m| wrap_diff(*m, cw)).collect::<Vec<_>>()).unwrap_or(0.0);
        out.push(Insight {
            id: "sleep-timing".into(),
            metric: Metric::Sleep,
            title: "Sleep timing".into(),
            body: format!(
                "Over {} nights you usually fell asleep around {} (typical variation ±{} min) and woke around {} (±{} min).",
                beds.len(),
                fmt_clock(cb),
                sd_b.round(),
                fmt_clock(cw),
                sd_w.round()
            ),
            window: window(w14.0, w14.1, beds.len()),
            baseline: None,
            direction: Direction::Info,
        });
    }

    // Activity consistency: days at or above the step goal.
    let series = idx.series(Metric::Steps, w14.0, w14.1 - Duration::days(1));
    let with: Vec<f64> = series.iter().filter_map(|(_, v)| *v).collect();
    if with.len() >= 7 {
        let met = with.iter().filter(|v| **v >= s.step_goal as f64).count();
        out.push(Insight {
            id: "step-consistency".into(),
            metric: Metric::Steps,
            title: "Step goal consistency".into(),
            body: format!(
                "You reached {} steps on {met} of the {} complete days with step data in the last two weeks.",
                fmt(Metric::Steps, s.step_goal as f64, s).replace(" steps", ""),
                with.len()
            ),
            window: window(w14.0, w14.1, with.len()),
            baseline: None,
            direction: Direction::Info,
        });
    }

    // Sleep duration against goal.
    let sleep: Vec<f64> = idx.series(Metric::Sleep, w14.0, w14.1).into_iter().filter_map(|(_, v)| v).collect();
    if sleep.len() >= 7 {
        let met = sleep.iter().filter(|v| **v >= s.sleep_goal_min as f64).count();
        out.push(Insight {
            id: "sleep-goal".into(),
            metric: Metric::Sleep,
            title: "Sleep goal".into(),
            body: format!(
                "{met} of {} recorded nights reached your sleep goal of {}.",
                sleep.len(),
                time::fmt_minutes(s.sleep_goal_min as f64)
            ),
            window: window(w14.0, w14.1, sleep.len()),
            baseline: None,
            direction: Direction::Info,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circular_mean_wraps_midnight() {
        let m = circular_mean_minutes(&[1430.0, 10.0]).unwrap();
        assert!(m < 1.0 || m > 1439.0, "{m}");
        assert!((wrap_diff(1430.0, 0.0) + 10.0).abs() < 1e-9);
    }

    #[test]
    fn insufficient_data_produces_no_insight() {
        let idx = SummaryIndex::default();
        let today = NaiveDate::from_ymd_opt(2026, 6, 1).unwrap();
        assert!(generate(&idx, today, &chrono_tz::Tz::UTC, &Settings::default()).is_empty());
    }
}
