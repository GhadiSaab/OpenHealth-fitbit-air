//! User settings and display formatting.
//!
//! [`Settings`] contains no secrets by construction. AI API keys live only in
//! platform secure storage (Android) or memory (browser) and are handled
//! outside this crate's serializable types, so they cannot leak into backups.

use crate::model::{Metric, Unit};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    #[default]
    Metric,
    Imperial,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TempUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum MotionPref {
    #[default]
    System,
    Reduced,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Reminders {
    /// Reminders are off unless the user turns them on.
    pub enabled: bool,
    /// Evening journal reminder, local time "HH:MM".
    pub journal_time: Option<String>,
    /// Wind-down reminder before the sleep goal bedtime, local time "HH:MM".
    pub bedtime_time: Option<String>,
}

impl Default for Reminders {
    fn default() -> Self {
        Reminders { enabled: false, journal_time: Some("21:00".into()), bedtime_time: None }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    pub theme: Theme,
    pub units: UnitSystem,
    pub temperature: TempUnit,
    pub motion: MotionPref,
    pub haptics: bool,
    /// IANA zone used for local days when a record has no offset.
    pub timezone: String,
    pub step_goal: u32,
    /// Sleep-duration goal in minutes.
    pub sleep_goal_min: u32,
    pub favorites: Vec<Metric>,
    pub reminders: Reminders,
    /// Has the user finished onboarding?
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            units: UnitSystem::Metric,
            temperature: TempUnit::Celsius,
            motion: MotionPref::System,
            haptics: true,
            timezone: "UTC".into(),
            step_goal: 8000,
            sleep_goal_min: 450,
            favorites: vec![Metric::RestingHeartRate, Metric::Hrv, Metric::Spo2, Metric::SkinTemperature],
            reminders: Reminders::default(),
            onboarded: false,
        }
    }
}

/// A value converted for display.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub number: String,
    pub unit: String,
}

impl Display {
    pub fn joined(&self) -> String {
        if self.unit.is_empty() {
            self.number.clone()
        } else if self.unit == "%" {
            format!("{}%", self.number)
        } else {
            format!("{}\u{a0}{}", self.number, self.unit)
        }
    }
}

fn group_thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Formats a canonical-unit value for display under the user's settings.
pub fn display(metric: Metric, value: f64, s: &Settings) -> Display {
    match metric.canonical_unit() {
        Unit::Steps => Display { number: group_thousands(value.round() as i64), unit: "steps".into() },
        Unit::Bpm => Display { number: format!("{}", value.round() as i64), unit: "bpm".into() },
        Unit::Minutes => {
            if metric == Metric::Sleep {
                Display { number: crate::time::fmt_minutes(value), unit: String::new() }
            } else {
                Display { number: format!("{}", value.round() as i64), unit: "min".into() }
            }
        }
        Unit::Meters => match s.units {
            UnitSystem::Metric => Display { number: format!("{:.2}", value / 1000.0), unit: "km".into() },
            UnitSystem::Imperial => Display { number: format!("{:.2}", value / 1609.344), unit: "mi".into() },
        },
        Unit::Kilocalories => Display { number: group_thousands(value.round() as i64), unit: "kcal".into() },
        Unit::Percent => Display { number: format!("{:.1}", value), unit: "%".into() },
        Unit::Milliseconds => Display { number: format!("{}", value.round() as i64), unit: "ms".into() },
        Unit::CelsiusDelta => {
            let (v, u) = match s.temperature {
                TempUnit::Celsius => (value, "°C"),
                // A temperature *difference* converts with 9/5 and no offset.
                TempUnit::Fahrenheit => (value * 9.0 / 5.0, "°F"),
            };
            let r = (v * 10.0).round() / 10.0;
            let sign = if r > 0.0 { "+" } else if r < 0.0 { "−" } else { "" };
            Display { number: format!("{sign}{:.1}", r.abs()), unit: u.into() }
        }
        Unit::BreathsPerMinute => Display { number: format!("{:.1}", value), unit: "br/min".into() },
    }
}

/// Converts a canonical value into the display scale (for chart axes).
pub fn display_scale(metric: Metric, value: f64, s: &Settings) -> f64 {
    match metric.canonical_unit() {
        Unit::Meters => match s.units {
            UnitSystem::Metric => value / 1000.0,
            UnitSystem::Imperial => value / 1609.344,
        },
        Unit::CelsiusDelta => match s.temperature {
            TempUnit::Celsius => value,
            TempUnit::Fahrenheit => value * 9.0 / 5.0,
        },
        Unit::Minutes if metric == Metric::Sleep => value / 60.0,
        _ => value,
    }
}

pub fn display_unit(metric: Metric, s: &Settings) -> &'static str {
    match metric.canonical_unit() {
        Unit::Meters => match s.units {
            UnitSystem::Metric => "km",
            UnitSystem::Imperial => "mi",
        },
        Unit::CelsiusDelta => match s.temperature {
            TempUnit::Celsius => "°C vs baseline",
            TempUnit::Fahrenheit => "°F vs baseline",
        },
        Unit::Minutes if metric == Metric::Sleep => "hours",
        Unit::Minutes => "min",
        Unit::Steps => "steps",
        Unit::Bpm => "bpm",
        Unit::Kilocalories => "kcal",
        Unit::Percent => "%",
        Unit::Milliseconds => "ms",
        Unit::BreathsPerMinute => "br/min",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperature_delta_converts_without_offset() {
        let mut s = Settings::default();
        s.temperature = TempUnit::Fahrenheit;
        assert_eq!(display(Metric::SkinTemperature, 1.0, &s).joined(), "+1.8\u{a0}°F");
        s.temperature = TempUnit::Celsius;
        assert_eq!(display(Metric::SkinTemperature, -0.4, &s).joined(), "−0.4\u{a0}°C");
        assert_eq!(display(Metric::SkinTemperature, -0.04, &s).joined(), "0.0\u{a0}°C");
    }

    #[test]
    fn distance_units() {
        let mut s = Settings::default();
        assert_eq!(display(Metric::Distance, 5000.0, &s).joined(), "5.00\u{a0}km");
        s.units = UnitSystem::Imperial;
        assert_eq!(display(Metric::Distance, 1609.344, &s).joined(), "1.00\u{a0}mi");
    }

    #[test]
    fn thousands() {
        assert_eq!(display(Metric::Steps, 12345.0, &Settings::default()).number, "12,345");
    }
}
