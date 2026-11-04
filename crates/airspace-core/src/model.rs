//! Core data model: measurements and their provenance.
//!
//! Every value Airspace stores is a [`Measurement`]. A measurement never stands
//! alone: it always carries a [`Provenance`] describing where it came from,
//! how it entered the database and how trustworthy its field mapping is.
//!
//! Timestamps are UTC milliseconds since the Unix epoch. The offset that was
//! in effect where the value was recorded is kept separately (when the source
//! provides it) so local days can be reconstructed faithfully after travel or
//! a daylight-saving change.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Schema version of a stored [`Measurement`]. Bump when the shape changes and
/// add a migration in the repository layer.
pub const MEASUREMENT_SCHEMA_VERSION: u32 = 1;

/// The eleven stored metrics, grouped into ten user-facing groups
/// (resting heart rate and heart-rate zones share a group).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    HeartRate,
    Steps,
    Sleep,
    RestingHeartRate,
    HeartRateZones,
    Distance,
    Calories,
    Spo2,
    Hrv,
    SkinTemperature,
    RespiratoryRate,
}

impl Metric {
    pub const ALL: [Metric; 11] = [
        Metric::HeartRate,
        Metric::Steps,
        Metric::Sleep,
        Metric::RestingHeartRate,
        Metric::HeartRateZones,
        Metric::Distance,
        Metric::Calories,
        Metric::Spo2,
        Metric::Hrv,
        Metric::SkinTemperature,
        Metric::RespiratoryRate,
    ];

    /// Stable machine identifier, identical to the serde representation.
    pub fn key(self) -> &'static str {
        match self {
            Metric::HeartRate => "heart_rate",
            Metric::Steps => "steps",
            Metric::Sleep => "sleep",
            Metric::RestingHeartRate => "resting_heart_rate",
            Metric::HeartRateZones => "heart_rate_zones",
            Metric::Distance => "distance",
            Metric::Calories => "calories",
            Metric::Spo2 => "spo2",
            Metric::Hrv => "hrv",
            Metric::SkinTemperature => "skin_temperature",
            Metric::RespiratoryRate => "respiratory_rate",
        }
    }

    pub fn from_key(s: &str) -> Option<Metric> {
        Metric::ALL.into_iter().find(|m| m.key() == s)
    }

    /// The canonical unit values of this metric are stored in.
    pub fn canonical_unit(self) -> Unit {
        match self {
            Metric::HeartRate | Metric::RestingHeartRate => Unit::Bpm,
            Metric::Steps => Unit::Steps,
            Metric::Sleep | Metric::HeartRateZones => Unit::Minutes,
            Metric::Distance => Unit::Meters,
            Metric::Calories => Unit::Kilocalories,
            Metric::Spo2 => Unit::Percent,
            Metric::Hrv => Unit::Milliseconds,
            Metric::SkinTemperature => Unit::CelsiusDelta,
            Metric::RespiratoryRate => Unit::BreathsPerMinute,
        }
    }

    /// Metrics whose daily value is a sum of parts (and therefore at risk of
    /// double counting when several sources overlap).
    pub fn is_additive(self) -> bool {
        matches!(self, Metric::Steps | Metric::Distance | Metric::Calories)
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Bpm,
    Steps,
    Minutes,
    Meters,
    Kilocalories,
    Percent,
    Milliseconds,
    /// A temperature *difference* in °C relative to a personal baseline.
    /// Never an absolute temperature.
    CelsiusDelta,
    BreathsPerMinute,
}

impl Unit {
    pub fn key(self) -> &'static str {
        match self {
            Unit::Bpm => "bpm",
            Unit::Steps => "steps",
            Unit::Minutes => "min",
            Unit::Meters => "m",
            Unit::Kilocalories => "kcal",
            Unit::Percent => "%",
            Unit::Milliseconds => "ms",
            Unit::CelsiusDelta => "delta_c",
            Unit::BreathsPerMinute => "br/min",
        }
    }

    /// Parses the unit spellings accepted by the import schema.
    pub fn parse(s: &str) -> Option<Unit> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "bpm" | "beats/min" => Unit::Bpm,
            "steps" | "count" => Unit::Steps,
            "min" | "minutes" => Unit::Minutes,
            "m" | "meters" | "metres" => Unit::Meters,
            "kcal" | "kilocalories" => Unit::Kilocalories,
            "%" | "percent" => Unit::Percent,
            "ms" | "milliseconds" => Unit::Milliseconds,
            "delta_c" | "°c delta" | "c_delta" => Unit::CelsiusDelta,
            "br/min" | "breaths/min" | "brpm" => Unit::BreathsPerMinute,
            _ => return None,
        })
    }
}

/// How a value relates to time.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SampleKind {
    /// A reading taken at `start` (e.g. one heart-rate sample).
    Instant,
    /// A quantity accumulated over `[start, end)` (e.g. steps in 15 minutes).
    Interval,
    /// A running device counter read at `start`. Counters may reset; the
    /// value is *not* an increment and must never be summed.
    CumulativeCounter,
    /// A source-provided total for one local day (`day`).
    DailyTotal,
    /// A session spanning `[start, end)` (sleep).
    Session,
}

/// Which pipeline brought the measurement into Airspace.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Imported from a history file (or simulated history in the demo).
    HistoryImport,
    /// Recorded live over Bluetooth (or simulated live data in the demo).
    BluetoothLive,
    /// Calculated by Airspace from other measurements.
    Derived,
}

/// Whether the value was measured by a device, imported as a source-reported
/// figure, or calculated.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DataStatus {
    Measured,
    Imported,
    Derived,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Provenance {
    pub origin: Origin,
    pub status: DataStatus,
    /// Human-readable source, e.g. "Airspace import v1" or "Fitbit Air (BLE)".
    pub source_name: String,
    /// Device name or model when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Identifier assigned by the original source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_id: Option<String>,
    /// Import batch that created this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    /// When Airspace stored the record (UTC ms).
    pub ingested_at: i64,
    /// True when the field mapping is not yet validated (e.g. provisional
    /// Bluetooth characteristic semantics).
    #[serde(default)]
    pub provisional: bool,
    /// Fraction of the covered interval with actual data, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// Source-reported quality/confidence (0–3 for Fitbit-style confidence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<u8>,
    /// Version of the Airspace calculation that produced a derived value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calc_version: Option<u32>,
    /// Marks records belonging to the demo database.
    #[serde(default)]
    pub demo: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SleepStage {
    Awake,
    Light,
    Deep,
    Rem,
    /// Asleep, but the source does not distinguish stages.
    Asleep,
    /// Restless (classic Fitbit sleep without stages).
    Restless,
}

impl SleepStage {
    pub fn is_asleep(self) -> bool {
        !matches!(self, SleepStage::Awake)
    }
    pub fn parse(s: &str) -> Option<SleepStage> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "awake" | "wake" => SleepStage::Awake,
            "light" => SleepStage::Light,
            "deep" => SleepStage::Deep,
            "rem" => SleepStage::Rem,
            "asleep" => SleepStage::Asleep,
            "restless" => SleepStage::Restless,
            _ => return None,
        })
    }
    pub fn key(self) -> &'static str {
        match self {
            SleepStage::Awake => "awake",
            SleepStage::Light => "light",
            SleepStage::Deep => "deep",
            SleepStage::Rem => "rem",
            SleepStage::Asleep => "asleep",
            SleepStage::Restless => "restless",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageInterval {
    pub stage: SleepStage,
    pub start: i64,
    pub end: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SleepDetail {
    /// Stage intervals as reported by the source. Empty when unavailable.
    #[serde(default)]
    pub stages: Vec<StageInterval>,
    /// True when the source reported stages (vs. only a start/end).
    pub stages_measured: bool,
    /// True when `value` (minutes asleep) was calculated by Airspace from
    /// stages rather than reported by the source.
    #[serde(default)]
    pub asleep_minutes_derived: bool,
    /// Main sleep vs. nap, when the source says so.
    #[serde(default = "default_true")]
    pub is_main: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HrvMethod {
    Rmssd,
    Sdnn,
    Unknown,
}

impl HrvMethod {
    pub fn label(self) -> &'static str {
        match self {
            HrvMethod::Rmssd => "RMSSD",
            HrvMethod::Sdnn => "SDNN",
            HrvMethod::Unknown => "unspecified method",
        }
    }
    pub fn parse(s: &str) -> HrvMethod {
        match s.trim().to_ascii_lowercase().as_str() {
            "rmssd" => HrvMethod::Rmssd,
            "sdnn" => HrvMethod::Sdnn,
            _ => HrvMethod::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CalorieScope {
    /// Total energy expenditure including basal metabolic rate.
    Total,
    /// Activity calories only.
    Active,
    /// The source does not say which.
    Unspecified,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum HrZone {
    BelowZones,
    FatBurn,
    Cardio,
    Peak,
}

impl HrZone {
    pub const ALL: [HrZone; 4] = [HrZone::BelowZones, HrZone::FatBurn, HrZone::Cardio, HrZone::Peak];
    pub fn label(self) -> &'static str {
        match self {
            HrZone::BelowZones => "Below zones",
            HrZone::FatBurn => "Fat burn",
            HrZone::Cardio => "Cardio",
            HrZone::Peak => "Peak",
        }
    }
    pub fn parse(s: &str) -> Option<HrZone> {
        Some(match s.trim().to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "below_zones" | "out_of_range" | "below" => HrZone::BelowZones,
            "fat_burn" | "fatburn" => HrZone::FatBurn,
            "cardio" => HrZone::Cardio,
            "peak" => HrZone::Peak,
            _ => return None,
        })
    }
}

/// Whether heart-rate zone boundaries came from the source or were computed.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ZoneOrigin {
    Imported,
    Calculated,
}

/// Metric-specific details that do not fit a single number.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Detail {
    Sleep(SleepDetail),
    Hrv { method: HrvMethod },
    Calories { scope: CalorieScope },
    Zone {
        zone: HrZone,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lower_bpm: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        upper_bpm: Option<f64>,
        origin: ZoneOrigin,
    },
    /// A value with a reported range (e.g. nightly SpO₂ average with min/max).
    Range { min: f64, max: f64 },
    /// Respiratory rate, optionally for a single sleep stage.
    Respiratory {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stage: Option<SleepStage>,
    },
    /// Cumulative counter metadata.
    Counter {
        /// The device resets the counter at local midnight.
        resets_daily: bool,
    },
}

impl Detail {
    /// A short discriminant used in conflict keys.
    pub fn discriminant(&self) -> String {
        match self {
            Detail::Sleep(_) => "sleep".into(),
            Detail::Hrv { method } => format!("hrv:{method:?}"),
            Detail::Calories { scope } => format!("cal:{scope:?}"),
            Detail::Zone { zone, .. } => format!("zone:{zone:?}"),
            Detail::Range { .. } => "range".into(),
            Detail::Respiratory { stage } => format!("resp:{stage:?}"),
            Detail::Counter { .. } => "counter".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Measurement {
    /// Deterministic content identifier (see [`crate::dedupe::measurement_id`]).
    pub id: String,
    pub metric: Metric,
    pub kind: SampleKind,
    /// UTC milliseconds.
    pub start: i64,
    /// UTC milliseconds, exclusive. Present for intervals, sessions and daily totals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
    /// Offset from UTC in minutes where the value was recorded, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub utc_offset_min: Option<i32>,
    /// Local calendar day for [`SampleKind::DailyTotal`] records (YYYY-MM-DD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<String>,
    pub value: f64,
    pub unit: Unit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<Detail>,
    pub prov: Provenance,
    /// Set when the user chose to prefer a different, conflicting record. The
    /// original is kept, never overwritten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

impl Measurement {
    pub fn is_active(&self) -> bool {
        self.superseded_by.is_none()
    }
    pub fn end_or_start(&self) -> i64 {
        self.end.unwrap_or(self.start)
    }
    pub fn sleep(&self) -> Option<&SleepDetail> {
        match &self.detail {
            Some(Detail::Sleep(s)) => Some(s),
            _ => None,
        }
    }
    pub fn hrv_method(&self) -> HrvMethod {
        match &self.detail {
            Some(Detail::Hrv { method }) => *method,
            _ => HrvMethod::Unknown,
        }
    }
    pub fn calorie_scope(&self) -> CalorieScope {
        match &self.detail {
            Some(Detail::Calories { scope }) => *scope,
            _ => CalorieScope::Unspecified,
        }
    }
}

/// A record of one import, kept so a user can see what came from where.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImportBatch {
    pub id: String,
    pub created_at: i64,
    pub adapter: String,
    pub file_name: String,
    pub added: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub conflict_policy: String,
    #[serde(default)]
    pub first_day: Option<String>,
    #[serde(default)]
    pub last_day: Option<String>,
}

/// Which database is active. Demo and personal data never share storage.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DbKind {
    Personal,
    Demo,
}

impl DbKind {
    pub fn key(self) -> &'static str {
        match self {
            DbKind::Personal => "personal",
            DbKind::Demo => "demo",
        }
    }
}
