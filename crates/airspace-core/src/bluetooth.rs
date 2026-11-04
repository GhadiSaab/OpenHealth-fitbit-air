//! Typed interface for a future Bluetooth live provider.
//!
//! **Nothing in this module talks to a device.** It defines the contract a
//! native Android service (see docs/BLUETOOTH_INTEGRATION.md) must satisfy,
//! plus the UI-facing state machine. This build ships only
//! [`NotConnectedLive`], which honestly reports that live recording is not
//! available.
//!
//! What is known (from separate research, still to be validated on Android):
//! a Bluetooth client has shown live heart rate, steps, distance and
//! calories. Field semantics (for example whether steps are a cumulative
//! counter and when it resets) are unconfirmed, so live values for those
//! metrics must be marked `provisional`. Sleep stages, SpO₂, HRV,
//! respiratory rate and temperature must **not** be inferred from that
//! stream.

use crate::model::{Metric, SampleKind};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ConnectionState {
    /// Live recording isn't part of this build or platform.
    Unavailable { reason: String },
    /// The user (or system) denied Bluetooth/nearby-devices permission.
    PermissionDenied { can_ask_again: bool },
    BluetoothOff,
    /// No known device has been set up.
    NotPaired,
    Disconnected { since: Option<i64> },
    Scanning,
    Connecting,
    Connected { since: i64 },
    Reconnecting { attempt: u32, since: i64 },
    OutOfRange { since: i64 },
    /// Device credentials expired or were revoked; re-authorization needed.
    AuthorizationExpired { message: String },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected { .. })
    }
    pub fn title(&self) -> &'static str {
        match self {
            ConnectionState::Unavailable { .. } => "Not connected",
            ConnectionState::PermissionDenied { .. } => "Bluetooth permission needed",
            ConnectionState::BluetoothOff => "Bluetooth is off",
            ConnectionState::NotPaired => "No device set up",
            ConnectionState::Disconnected { .. } => "Disconnected",
            ConnectionState::Scanning => "Looking for your tracker",
            ConnectionState::Connecting => "Connecting",
            ConnectionState::Connected { .. } => "Connected",
            ConnectionState::Reconnecting { .. } => "Reconnecting",
            ConnectionState::OutOfRange { .. } => "Out of range",
            ConnectionState::AuthorizationExpired { .. } => "Authorization expired",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub firmware: Option<String>,
    /// Battery percentage, only when the device reports it.
    #[serde(default)]
    pub battery_pct: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Demonstrated and validated.
    Supported,
    /// Demonstrated, but field semantics are unconfirmed.
    Provisional,
    /// Not available from the live stream; history import only.
    Unsupported,
}

/// What the live stream can provide, per metric. Anything not listed
/// `Supported`/`Provisional` must come from history import.
pub fn live_capabilities() -> Vec<(Metric, Support)> {
    use Metric::*;
    vec![
        (HeartRate, Support::Provisional),
        (Steps, Support::Provisional),
        (Distance, Support::Provisional),
        (Calories, Support::Provisional),
        (Sleep, Support::Unsupported),
        (RestingHeartRate, Support::Unsupported),
        (HeartRateZones, Support::Unsupported),
        (Spo2, Support::Unsupported),
        (Hrv, Support::Unsupported),
        (SkinTemperature, Support::Unsupported),
        (RespiratoryRate, Support::Unsupported),
    ]
}

/// One timestamped live reading as delivered by the native service.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LiveSample {
    pub metric: Metric,
    /// `Instant` for heart rate; `CumulativeCounter` for steps/distance/calories
    /// until validated otherwise.
    pub kind: SampleKind,
    /// Device or phone receive time, UTC ms (see `time_basis`).
    pub ts: i64,
    pub value: f64,
    pub provisional: bool,
    pub time_basis: TimeBasis,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeBasis {
    /// Timestamp provided by the device clock.
    Device,
    /// Time the phone received the notification.
    PhoneReceive,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RecordingGap {
    pub start: i64,
    pub end: i64,
    pub reason: GapReason,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GapReason {
    ConnectionLost,
    OutOfRange,
    AppKilled,
    BluetoothOff,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RecordingSession {
    pub id: String,
    pub device: DeviceInfo,
    pub started_at: i64,
    #[serde(default)]
    pub ended_at: Option<i64>,
    pub gaps: Vec<RecordingGap>,
    pub samples: u64,
}

/// Events emitted by a live provider.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum LiveEvent {
    State(ConnectionState),
    Device(DeviceInfo),
    Sample(LiveSample),
    SessionStarted(RecordingSession),
    SessionEnded { id: String, at: i64 },
    Reconnected { session_id: String, gap: RecordingGap },
    Gap(RecordingGap),
    Unsupported { metric: Metric, message: String },
    AuthError { message: String },
}

/// Contract for a live provider. A native implementation streams
/// [`LiveEvent`]s to the UI and persists samples as `BluetoothLive`
/// measurements.
pub trait LiveProvider {
    fn state(&self) -> ConnectionState;
    fn device(&self) -> Option<DeviceInfo>;
    /// Starts recording. Must fail with a meaningful state, never pretend.
    fn start(&mut self) -> Result<(), ConnectionState>;
    fn stop(&mut self);
}

/// The only live provider in this build.
pub struct NotConnectedLive;

impl LiveProvider for NotConnectedLive {
    fn state(&self) -> ConnectionState {
        ConnectionState::Unavailable {
            reason: "Bluetooth recording isn't part of this version of Airspace. It will arrive as a native Android service.".into(),
        }
    }
    fn device(&self) -> Option<DeviceInfo> {
        None
    }
    fn start(&mut self) -> Result<(), ConnectionState> {
        Err(self.state())
    }
    fn stop(&mut self) {}
}

/// Freshness of the most recent reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Arrived within the last two minutes while connected.
    Live,
    /// Within the last 30 minutes.
    Recent,
    /// Older than 30 minutes.
    Stale,
    /// From imported history, not the live stream.
    Historical,
}

pub fn freshness(ts: i64, now: i64, from_live: bool, connected: bool) -> Freshness {
    if !from_live {
        return Freshness::Historical;
    }
    let age = now - ts;
    if connected && age <= 120_000 {
        Freshness::Live
    } else if age <= 30 * 60_000 {
        Freshness::Recent
    } else {
        Freshness::Stale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_connected_provider_never_pretends() {
        let mut p = NotConnectedLive;
        assert!(p.start().is_err());
        assert!(!p.state().is_connected());
        assert_eq!(p.state().title(), "Not connected");
    }

    #[test]
    fn freshness_rules() {
        assert_eq!(freshness(0, 60_000, true, true), Freshness::Live);
        assert_eq!(freshness(0, 60_000, true, false), Freshness::Recent);
        assert_eq!(freshness(0, 3_600_000, true, false), Freshness::Stale);
        assert_eq!(freshness(0, 1, false, true), Freshness::Historical);
    }

    #[test]
    fn advanced_metrics_are_not_live() {
        for (m, s) in live_capabilities() {
            if matches!(m, Metric::Sleep | Metric::Spo2 | Metric::Hrv | Metric::RespiratoryRate | Metric::SkinTemperature) {
                assert_eq!(s, Support::Unsupported);
            }
        }
    }
}
