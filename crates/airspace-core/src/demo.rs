//! Deterministic 30-day demo dataset.
//!
//! The same `(end_day, time zone, now)` always produces the same data. It
//! covers every metric and deliberately includes:
//! * a day with no data at all (tracker not worn),
//! * a day with a multi-hour gap (charging),
//! * a night without sleep stages,
//! * HRV values measured with a different method (kept separate),
//! * today's history ending at a sync point, followed by a simulated live
//!   Bluetooth session that disconnected a while ago (stale readings).
//!
//! Everything is marked `demo: true` and only ever written to the demo database.

use crate::dedupe::measurement_id;
use crate::journal::JournalEntry;
use crate::model::*;
use crate::time::{self, MINUTE_MS};
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

pub const DEMO_DAYS: i64 = 30;
pub const MISSING_DAY: i64 = 11;
pub const CHARGING_DAY: i64 = 19;
pub const UNSTAGED_NIGHT: i64 = 7;

const HISTORY_SOURCE: &str = "Demo history (simulated import)";
const LIVE_SOURCE: &str = "Demo live stream (simulated Bluetooth)";
const OTHER_SOURCE: &str = "Demo: other app export";

/// SplitMix64: tiny, fast, deterministic.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Approximately normal via the sum of uniforms.
    fn n(&mut self, mean: f64, sd: f64) -> f64 {
        let s: f64 = (0..6).map(|_| self.f()).sum::<f64>() - 3.0;
        mean + s * sd / 0.7071
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DemoDevice {
    pub name: String,
    pub battery_pct: u8,
    /// When the simulated live connection dropped.
    pub disconnected_at: i64,
    /// When the simulated live session began.
    pub session_started_at: i64,
    /// Last imported-history sync point.
    pub history_synced_through: i64,
}

#[derive(Clone, Debug)]
pub struct DemoDataset {
    pub measurements: Vec<Measurement>,
    pub journal: Vec<JournalEntry>,
    pub device: DemoDevice,
}

struct Builder<'a> {
    out: Vec<Measurement>,
    tz: &'a Tz,
    now: i64,
}

impl Builder<'_> {
    fn push(&mut self, metric: Metric, kind: SampleKind, start: i64, end: Option<i64>, value: f64, detail: Option<Detail>, origin: Origin, source: &str, provisional: bool) {
        let mut m = Measurement {
            id: String::new(),
            metric,
            kind,
            start,
            end,
            utc_offset_min: Some(time::offset_minutes_at(start, self.tz)),
            day: None,
            value,
            unit: metric.canonical_unit(),
            detail,
            prov: Provenance {
                origin,
                status: if origin == Origin::BluetoothLive { DataStatus::Measured } else { DataStatus::Imported },
                source_name: source.into(),
                device: Some("Fitbit Air (demo)".into()),
                original_id: None,
                batch_id: Some("demo".into()),
                ingested_at: self.now,
                provisional,
                coverage: None,
                confidence: None,
                calc_version: None,
                demo: true,
            },
            superseded_by: None,
        };
        if kind == SampleKind::DailyTotal {
            m.day = Some(time::day_key(time::local_day_in(start, self.tz)));
        }
        m.id = measurement_id(&m);
        self.out.push(m);
    }
    fn daily(&mut self, metric: Metric, day: NaiveDate, value: f64, detail: Option<Detail>) {
        let (a, b) = time::day_bounds(day, self.tz);
        self.push(metric, SampleKind::DailyTotal, a, Some(b), value, detail, Origin::HistoryImport, HISTORY_SOURCE, false);
    }
    fn nightly(&mut self, metric: Metric, ts: i64, value: f64, detail: Option<Detail>) {
        self.push(metric, SampleKind::Instant, ts, None, value, detail, Origin::HistoryImport, HISTORY_SOURCE, false);
    }
}

fn is_workout(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Mon | Weekday::Wed | Weekday::Sat)
}

/// Generates the dataset ending on `end_day` (normally today).
pub fn generate(end_day: NaiveDate, tz: &Tz, now: i64) -> DemoDataset {
    let now = now - now.rem_euclid(MINUTE_MS);
    let seed = (end_day.num_days_from_ce() as u64).wrapping_mul(0x2545F4914F6CDD1D) ^ 0xA1A5_FACE;
    let mut rng = Rng(seed);
    let mut b = Builder { out: Vec::with_capacity(60_000), tz, now };
    let first = end_day - Duration::days(DEMO_DAYS - 1);
    let (today_start, _) = time::day_bounds(end_day, tz);

    // Simulated live session on the last day: history synced through `sync`,
    // then live recording until `disconnected`.
    let disconnected = (now - 134 * MINUTE_MS).max(today_start + 30 * MINUTE_MS);
    let sync = (now - 6 * 60 * MINUTE_MS).max(today_start).min(disconnected - 20 * MINUTE_MS);

    let mut journal = vec![];
    let alcohol = [5, 12, 18, 25];
    let illness = [21, 22];
    let caffeine_late = [3, 9, 16, 27];
    let travel = [14, 15];

    for i in 0..DEMO_DAYS {
        let day = first + Duration::days(i);
        let (ds, de) = time::day_bounds(day, tz);
        let workout = is_workout(day) && !illness.contains(&i);
        let ill = illness.contains(&i);
        let after_alcohol = alcohol.contains(&(i - 1));
        let after_ill = ill || illness.contains(&(i - 1));
        let weekend = matches!(day.weekday(), Weekday::Sat | Weekday::Sun);
        let is_today = day == end_day;

        // Journal (including the day with no data).
        let mut tags = vec![];
        let mut note = String::new();
        if workout {
            tags.push("exercise".to_string());
            note = ["Evening run, felt steady.", "Strength session at the gym.", "Long walk with a hill section."][(i % 3) as usize].into();
        }
        if alcohol.contains(&i) {
            tags.push("alcohol".into());
            note = "Dinner with friends, two glasses of wine.".into();
        }
        if caffeine_late.contains(&i) {
            tags.push("caffeine".into());
        }
        if travel.contains(&i) {
            tags.push("travel".into());
            note = "Train trip, a lot of sitting.".into();
        }
        if ill {
            tags.push("illness".into());
            note = "Sore throat, stayed home.".into();
        }
        if i == 9 {
            tags.push("stress".into());
            note = "Deadline at work.".into();
        }
        if i == MISSING_DAY {
            note = "Forgot to put the tracker back on after charging.".into();
        }
        if !tags.is_empty() || !note.is_empty() {
            let mut e = JournalEntry::new(day, now);
            e.tags = tags;
            e.note = note;
            e.demo = true;
            journal.push(e);
        }

        if i == MISSING_DAY {
            continue;
        }

        // ---- Sleep ending this morning ------------------------------------
        // The night after the missing day is missing too: the tracker wasn't worn.
        {
            let late = if weekend { 35.0 } else { 0.0 } + if caffeine_late.contains(&(i - 1)) { 40.0 } else { 0.0 };
            let bed_min = rng.n(-45.0 + late, 20.0); // minutes relative to midnight
            let wake_min = rng.n(if weekend { 470.0 } else { 420.0 }, 18.0);
            let bed = ds + (bed_min * MINUTE_MS as f64) as i64;
            let wake = ds + (wake_min * MINUTE_MS as f64) as i64;
            if wake > bed && !(is_today && wake > now) && i != MISSING_DAY + 1 {
                let mut stages = vec![];
                let staged = i != UNSTAGED_NIGHT;
                let mut t = bed + (rng.n(14.0, 4.0).max(3.0) as i64) * MINUTE_MS;
                stages.push(StageInterval { stage: SleepStage::Awake, start: bed, end: t });
                let mut cycle = 0;
                while t < wake - 10 * MINUTE_MS {
                    let deep_scale = if after_alcohol { 0.6 } else { 1.0 } * if cycle < 2 { 1.0 } else { 0.35 };
                    let rem_scale = if after_alcohol { 0.5 } else { 1.0 } * if cycle < 2 { 0.6 } else { 1.3 };
                    let seq = [
                        (SleepStage::Light, rng.n(32.0, 6.0)),
                        (SleepStage::Deep, rng.n(28.0, 6.0) * deep_scale),
                        (SleepStage::Light, rng.n(14.0, 4.0)),
                        (SleepStage::Rem, rng.n(22.0, 5.0) * rem_scale),
                        (SleepStage::Awake, if rng.f() < 0.5 { rng.n(3.0, 1.0) } else { 0.0 }),
                    ];
                    for (stage, mins) in seq {
                        if mins < 1.0 {
                            continue;
                        }
                        let e = (t + (mins * MINUTE_MS as f64) as i64).min(wake);
                        if e > t {
                            stages.push(StageInterval { stage, start: t, end: e });
                        }
                        t = e;
                        if t >= wake {
                            break;
                        }
                    }
                    cycle += 1;
                }
                if t < wake {
                    stages.push(StageInterval { stage: SleepStage::Awake, start: t, end: wake });
                }
                let asleep: f64 = stages.iter().filter(|s| s.stage.is_asleep()).map(|s| (s.end - s.start) as f64 / MINUTE_MS as f64).sum();
                let detail = SleepDetail {
                    stages: if staged { stages } else { vec![] },
                    stages_measured: staged,
                    asleep_minutes_derived: false,
                    is_main: true,
                };
                b.push(Metric::Sleep, SampleKind::Session, bed, Some(wake), asleep.round(), Some(Detail::Sleep(detail)), Origin::HistoryImport, HISTORY_SOURCE, false);

                // Overnight metrics, timestamped at wake.
                let rhr = rng.n(56.0, 1.2) + if after_alcohol { 4.0 } else { 0.0 } + if after_ill { 5.0 } else { 0.0 };
                b.nightly(Metric::RestingHeartRate, wake, rhr.round(), None);
                let hrv = rng.n(44.0, 4.0) - if after_alcohol { 9.0 } else { 0.0 } - if after_ill { 12.0 } else { 0.0 };
                b.nightly(Metric::Hrv, wake, hrv.round(), Some(Detail::Hrv { method: HrvMethod::Rmssd }));
                if i == 3 || i == 4 {
                    b.push(Metric::Hrv, SampleKind::Instant, wake + MINUTE_MS, None, (hrv * 1.9).round(), Some(Detail::Hrv { method: HrvMethod::Sdnn }), Origin::HistoryImport, OTHER_SOURCE, false);
                }
                let spo2 = (rng.n(95.6, 0.5) * 10.0).round() / 10.0;
                b.nightly(Metric::Spo2, wake, spo2, Some(Detail::Range { min: (spo2 - rng.n(2.5, 0.6)).round(), max: (spo2 + 2.0).min(100.0).round() }));
                let temp = rng.n(0.0, 0.15) + if after_ill { 0.85 } else { 0.0 } + if after_alcohol { 0.25 } else { 0.0 };
                b.nightly(Metric::SkinTemperature, wake, (temp * 10.0).round() / 10.0, None);
                let rr = rng.n(14.6, 0.35) + if after_ill { 1.4 } else { 0.0 };
                let rr = (rr * 10.0).round() / 10.0;
                b.nightly(Metric::RespiratoryRate, wake, rr, Some(Detail::Respiratory { stage: None }));
                if staged {
                    for (st, d) in [(SleepStage::Deep, -0.7), (SleepStage::Light, 0.1), (SleepStage::Rem, 0.6)] {
                        b.nightly(Metric::RespiratoryRate, wake, ((rr + d) * 10.0).round() / 10.0, Some(Detail::Respiratory { stage: Some(st) }));
                    }
                }
            }
        }

        // ---- Intraday heart rate and steps --------------------------------
        let workout_start = ds + (if weekend { 10 * 60 } else { 18 * 60 + 10 }) * MINUTE_MS;
        let workout_end = workout_start + 42 * MINUTE_MS;
        let charge = (ds + 13 * 60 * MINUTE_MS, ds + (16 * 60 + 30) * MINUTE_MS);
        let day_end = if is_today { disconnected } else { de };
        let mut t = ds;
        let mut hr = 58.0;
        while t < day_end {
            if i == CHARGING_DAY && t >= charge.0 && t < charge.1 {
                t += MINUTE_MS;
                continue;
            }
            let m = (t - ds) as f64 / MINUTE_MS as f64;
            let asleep = !(420.0..1380.0).contains(&m);
            let target = if t >= workout_start && t < workout_end && workout {
                138.0 + 14.0 * ((t - workout_start) as f64 / (42.0 * MINUTE_MS as f64) * std::f64::consts::PI).sin()
            } else if asleep {
                54.0 + 3.0 * ((m / 90.0) * std::f64::consts::TAU).sin()
            } else {
                let commute = (480.0..525.0).contains(&m) || (1050.0..1090.0).contains(&m);
                (if commute && !weekend { 96.0 } else { 71.0 }) + if ill { 7.0 } else { 0.0 } + if i == 9 && (600.0..1000.0).contains(&m) { 8.0 } else { 0.0 }
            };
            hr += (target - hr) * 0.25 + rng.n(0.0, 1.6);
            let live = is_today && t >= sync;
            let (origin, source) = if live { (Origin::BluetoothLive, LIVE_SOURCE) } else { (Origin::HistoryImport, HISTORY_SOURCE) };
            b.push(Metric::HeartRate, SampleKind::Instant, t, None, hr.round().clamp(40.0, 190.0), None, origin, source, false);
            t += MINUTE_MS;
        }

        // Steps, distance and calories in 15-minute intervals (history) …
        let stride = 0.76;
        let mut step_total = 0.0;
        let mut counter = 0.0;
        let mut q = ds;
        while q < day_end {
            let qe = (q + 15 * MINUTE_MS).min(de);
            let m = (q - ds) as f64 / MINUTE_MS as f64;
            let in_charge = i == CHARGING_DAY && q >= charge.0 && q < charge.1;
            let awake_hours = (420.0..1380.0).contains(&m);
            let mut steps = if awake_hours && !in_charge {
                let base = if ill { 15.0 } else if travel.contains(&i) { 35.0 } else { 70.0 };
                let commute = ((480.0..525.0).contains(&m) || (1050.0..1090.0).contains(&m)) && !weekend;
                let lunch = (750.0..795.0).contains(&m);
                (base + if commute { 520.0 } else { 0.0 } + if lunch { 420.0 } else { 0.0 }) * rng.n(1.0, 0.35).max(0.0)
            } else {
                0.0
            };
            if workout && q >= workout_start - 15 * MINUTE_MS && q < workout_end {
                steps += 1100.0 * rng.n(1.0, 0.1);
            }
            let steps = steps.round();
            let live = is_today && q >= sync;
            if live {
                // … and as a provisional cumulative counter from the live stream.
                counter += steps;
                let read_at = qe.min(disconnected);
                b.push(Metric::Steps, SampleKind::CumulativeCounter, read_at, None, counter, Some(Detail::Counter { resets_daily: false }), Origin::BluetoothLive, LIVE_SOURCE, true);
                b.push(Metric::Distance, SampleKind::CumulativeCounter, read_at, None, (counter * stride).round(), Some(Detail::Counter { resets_daily: false }), Origin::BluetoothLive, LIVE_SOURCE, true);
                b.push(Metric::Calories, SampleKind::CumulativeCounter, read_at, None, (counter * 0.045).round(), Some(Detail::Calories { scope: CalorieScope::Unspecified }), Origin::BluetoothLive, LIVE_SOURCE, true);
            } else if !in_charge {
                b.push(Metric::Steps, SampleKind::Interval, q, Some(qe), steps, None, Origin::HistoryImport, HISTORY_SOURCE, false);
                // Zero intervals too, so distance has the same coverage as steps.
                b.push(Metric::Distance, SampleKind::Interval, q, Some(qe), (steps * stride).round(), None, Origin::HistoryImport, HISTORY_SOURCE, false);
                step_total += steps;
            }
            q = qe;
        }
        // First live counter reading establishes the baseline at the sync point.
        if is_today {
            b.push(Metric::Steps, SampleKind::CumulativeCounter, sync, None, 0.0, Some(Detail::Counter { resets_daily: false }), Origin::BluetoothLive, LIVE_SOURCE, true);
            b.push(Metric::Distance, SampleKind::CumulativeCounter, sync, None, 0.0, Some(Detail::Counter { resets_daily: false }), Origin::BluetoothLive, LIVE_SOURCE, true);
            b.push(Metric::Calories, SampleKind::CumulativeCounter, sync, None, 0.0, Some(Detail::Calories { scope: CalorieScope::Unspecified }), Origin::BluetoothLive, LIVE_SOURCE, true);
        }

        // Daily calories and heart-rate zones (complete days only).
        if !is_today {
            let active = (step_total * 0.045 + if workout { 320.0 } else { 0.0 }).round();
            b.daily(Metric::Calories, day, (1690.0 + active).round(), Some(Detail::Calories { scope: CalorieScope::Total }));
            b.daily(Metric::Calories, day, active, Some(Detail::Calories { scope: CalorieScope::Active }));
            let bounds = [(HrZone::FatBurn, 98.0, 123.0), (HrZone::Cardio, 123.0, 150.0), (HrZone::Peak, 150.0, 220.0)];
            let mins = if workout { [rng.n(30.0, 6.0), rng.n(26.0, 5.0), rng.n(4.0, 2.0)] } else { [rng.n(24.0, 8.0), rng.n(2.0, 2.0), 0.0] };
            for ((zone, lo, hi), mn) in bounds.into_iter().zip(mins) {
                b.daily(Metric::HeartRateZones, day, mn.max(0.0).round(), Some(Detail::Zone { zone, lower_bpm: Some(lo), upper_bpm: Some(hi), origin: ZoneOrigin::Imported }));
            }
        }
    }

    DemoDataset {
        measurements: b.out,
        journal,
        device: DemoDevice {
            name: "Fitbit Air (demo)".into(),
            battery_pct: 64,
            disconnected_at: disconnected,
            session_started_at: sync,
            history_synced_through: sync,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::{SummaryContext, SummaryIndex, summarize};

    fn fixture() -> (DemoDataset, NaiveDate, i64) {
        let tz = Tz::Europe__Berlin;
        let end = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let now = time::day_start(end, &tz) + 15 * 60 * MINUTE_MS;
        (generate(end, &tz, now), end, now)
    }

    #[test]
    fn deterministic() {
        let (a, _, _) = fixture();
        let (b, _, _) = fixture();
        assert_eq!(a.measurements, b.measurements);
    }

    #[test]
    fn covers_every_metric_and_is_marked_demo() {
        let (d, _, _) = fixture();
        for m in Metric::ALL {
            assert!(d.measurements.iter().any(|x| x.metric == m), "missing {m}");
        }
        assert!(d.measurements.iter().all(|m| m.prov.demo));
        assert!(d.journal.iter().all(|j| j.demo));
    }

    #[test]
    fn missing_day_has_no_summaries() {
        let (d, end, now) = fixture();
        let tz = Tz::Europe__Berlin;
        let idx = SummaryIndex::from_vec(summarize(&d.measurements, &SummaryContext { tz, now, demo: true }));
        let missing = end - Duration::days(DEMO_DAYS - 1 - MISSING_DAY);
        for m in Metric::ALL {
            assert!(idx.get(m, missing).is_none(), "{m} has data on the missing day");
        }
        // Steps on a normal day are plausible.
        let v = idx.value(Metric::Steps, end - Duration::days(2)).unwrap();
        assert!((2000.0..30000.0).contains(&v), "{v}");
        // Today combines history and live counter without double counting.
        let today = idx.get(Metric::Steps, end).unwrap();
        assert!(today.method.contains("Combined"), "{}", today.method);
        assert!(today.provisional);
    }

    #[test]
    fn ids_are_unique() {
        let (d, _, _) = fixture();
        let mut ids: Vec<&str> = d.measurements.iter().map(|m| m.id.as_str()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
