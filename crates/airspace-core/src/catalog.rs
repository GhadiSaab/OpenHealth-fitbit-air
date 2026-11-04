//! User-facing metric catalog: names, groups, explanations and display rules.

use crate::model::Metric;
use serde::{Deserialize, Serialize};

/// The ten metric groups shown in Explore.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    HeartRate,
    Steps,
    Sleep,
    RestingAndZones,
    Distance,
    Calories,
    Spo2,
    Hrv,
    SkinTemperature,
    RespiratoryRate,
}

impl Group {
    pub const ALL: [Group; 10] = [
        Group::HeartRate,
        Group::Steps,
        Group::Sleep,
        Group::RestingAndZones,
        Group::Distance,
        Group::Calories,
        Group::Spo2,
        Group::Hrv,
        Group::SkinTemperature,
        Group::RespiratoryRate,
    ];
    pub fn metrics(self) -> &'static [Metric] {
        match self {
            Group::HeartRate => &[Metric::HeartRate],
            Group::Steps => &[Metric::Steps],
            Group::Sleep => &[Metric::Sleep],
            Group::RestingAndZones => &[Metric::RestingHeartRate, Metric::HeartRateZones],
            Group::Distance => &[Metric::Distance],
            Group::Calories => &[Metric::Calories],
            Group::Spo2 => &[Metric::Spo2],
            Group::Hrv => &[Metric::Hrv],
            Group::SkinTemperature => &[Metric::SkinTemperature],
            Group::RespiratoryRate => &[Metric::RespiratoryRate],
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Group::HeartRate => "Heart rate",
            Group::Steps => "Steps & movement",
            Group::Sleep => "Sleep",
            Group::RestingAndZones => "Resting heart rate & zones",
            Group::Distance => "Distance",
            Group::Calories => "Calories",
            Group::Spo2 => "Blood oxygen (SpO₂)",
            Group::Hrv => "Heart-rate variability",
            Group::SkinTemperature => "Skin temperature",
            Group::RespiratoryRate => "Respiratory rate",
        }
    }
}

/// Visual family (maps to a color token in the UI).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Activity,
    Sleep,
    Heart,
    Oxygen,
    Variability,
    Temperature,
    Breath,
}

impl Tone {
    pub fn css(self) -> &'static str {
        match self {
            Tone::Activity => "activity",
            Tone::Sleep => "sleep",
            Tone::Heart => "heart",
            Tone::Oxygen => "oxygen",
            Tone::Variability => "variability",
            Tone::Temperature => "temperature",
            Tone::Breath => "breath",
        }
    }
}

/// How the metric's daily value should be charted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DailyChart {
    /// Bars that grow from zero (totals).
    Bars,
    /// Points connected within runs of consecutive days (levels).
    Line,
    /// Points with a min–max whisker.
    RangeLine,
    /// Deviation around a zero baseline.
    Deviation,
}

pub struct MetricInfo {
    pub metric: Metric,
    pub title: &'static str,
    pub short: &'static str,
    pub tone: Tone,
    pub group: Group,
    pub daily_chart: DailyChart,
    /// True when an intraday (day view) series exists for this metric.
    pub has_intraday: bool,
    pub explanation: &'static str,
    pub caution: &'static str,
}

pub fn info(metric: Metric) -> MetricInfo {
    use Metric::*;
    match metric {
        HeartRate => MetricInfo {
            metric,
            title: "Heart rate",
            short: "Heart rate",
            tone: Tone::Heart,
            group: Group::HeartRate,
            daily_chart: DailyChart::RangeLine,
            has_intraday: true,
            explanation: "Beats per minute measured by the optical sensor on your wrist. Heart rate rises with movement, stress, caffeine and warmth, and falls during rest and sleep. Gaps appear when the tracker was off-wrist, charging or disconnected.",
            caution: "Optical readings can be less accurate during vigorous wrist movement. Airspace does not detect arrhythmias.",
        },
        Steps => MetricInfo {
            metric,
            title: "Steps",
            short: "Steps",
            tone: Tone::Activity,
            group: Group::Steps,
            daily_chart: DailyChart::Bars,
            has_intraday: true,
            explanation: "Steps counted by the tracker's motion sensor. Daily totals come either from a source-reported total, from interval counts, or from the changes in the device's running step counter — never by adding counter readings together.",
            caution: "Arm movement without walking can add steps, and pushing a cart can hide them.",
        },
        Sleep => MetricInfo {
            metric,
            title: "Sleep",
            short: "Sleep",
            tone: Tone::Sleep,
            group: Group::Sleep,
            daily_chart: DailyChart::Bars,
            has_intraday: true,
            explanation: "Sleep sessions with the start and end times reported by the source. When the source includes stages, the timeline shows awake, light, deep and REM periods. Stage totals are calculated by Airspace from those intervals. A night is shown on the day you woke up.",
            caution: "Wrist-based stage estimates are approximations, not a sleep-lab measurement.",
        },
        RestingHeartRate => MetricInfo {
            metric,
            title: "Resting heart rate",
            short: "Resting HR",
            tone: Tone::Heart,
            group: Group::RestingAndZones,
            daily_chart: DailyChart::Line,
            has_intraday: false,
            explanation: "A daily estimate of your heart rate while at rest, as reported by the source. It tends to be stable for a person; changes of a few beats can follow hard training, illness, alcohol, heat or poor sleep.",
            caution: "Airspace shows the value provided by the source and does not compute its own resting heart rate.",
        },
        HeartRateZones => MetricInfo {
            metric,
            title: "Heart-rate zones",
            short: "Zone minutes",
            tone: Tone::Heart,
            group: Group::RestingAndZones,
            daily_chart: DailyChart::Bars,
            has_intraday: false,
            explanation: "Minutes spent in heart-rate zones (fat burn, cardio, peak). The value shown is the minutes above the lowest zone. Each record says whether the zone boundaries were imported from the source or calculated.",
            caution: "Zone boundaries are estimates based on age or resting heart rate and differ between sources.",
        },
        Distance => MetricInfo {
            metric,
            title: "Distance",
            short: "Distance",
            tone: Tone::Activity,
            group: Group::Distance,
            daily_chart: DailyChart::Bars,
            has_intraday: true,
            explanation: "Distance estimated from your steps and stride length. The tracker has no GPS, so Airspace never shows routes.",
            caution: "Stride-based distance is an estimate and can differ from a measured course.",
        },
        Calories => MetricInfo {
            metric,
            title: "Calories",
            short: "Calories",
            tone: Tone::Activity,
            group: Group::Calories,
            daily_chart: DailyChart::Bars,
            has_intraday: true,
            explanation: "Energy expenditure in kilocalories. Airspace only separates total and active calories when the source does. Live Bluetooth calorie values use a provisional field mapping and are marked as such.",
            caution: "Calorie estimates from wrist trackers have wide error margins.",
        },
        Spo2 => MetricInfo {
            metric,
            title: "Blood oxygen (SpO₂)",
            short: "SpO₂",
            tone: Tone::Oxygen,
            group: Group::Spo2,
            daily_chart: DailyChart::RangeLine,
            has_intraday: false,
            explanation: "Estimated oxygen saturation of your blood while you sleep, in percent. Values are imported from the source; Airspace does not derive SpO₂ from the live heart-rate stream.",
            caution: "Not a medical pulse oximeter. If you have concerns about your breathing or oxygen levels, talk to a clinician.",
        },
        Hrv => MetricInfo {
            metric,
            title: "Heart-rate variability",
            short: "HRV",
            tone: Tone::Variability,
            group: Group::Hrv,
            daily_chart: DailyChart::Line,
            has_intraday: false,
            explanation: "Variation in time between heartbeats, in milliseconds, usually measured during sleep. Airspace keeps the measurement method (such as RMSSD) and never combines values measured with different methods.",
            caution: "HRV varies widely between people; compare against your own baseline, not other people.",
        },
        SkinTemperature => MetricInfo {
            metric,
            title: "Skin temperature",
            short: "Skin temp",
            tone: Tone::Temperature,
            group: Group::SkinTemperature,
            daily_chart: DailyChart::Deviation,
            has_intraday: false,
            explanation: "How your wrist skin temperature during sleep differed from your personal baseline. This is a difference, not an absolute temperature, and it is not your core body temperature.",
            caution: "Room temperature, bedding and how the tracker is worn all affect skin temperature.",
        },
        RespiratoryRate => MetricInfo {
            metric,
            title: "Respiratory rate",
            short: "Breathing",
            tone: Tone::Breath,
            group: Group::RespiratoryRate,
            daily_chart: DailyChart::Line,
            has_intraday: false,
            explanation: "Average breaths per minute during sleep, as reported by the source, with per-stage values when available.",
            caution: "Estimated from heart-rate signals; not a clinical measurement.",
        },
    }
}
