//! Validated native live envelopes. Unknown fields are never health metrics.
use crate::{dedupe::measurement_id, model::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawLive {
    pub session: String,
    pub seq: u64,
    #[serde(default)]
    pub segment: u32,
    pub received_at: i64,
    pub utc_offset_min: i32,
    pub fields: BTreeMap<u32, u64>,
}
pub fn normalize(raw: &RawLive) -> Result<Vec<Measurement>, String> {
    if raw.session.is_empty()
        || raw.session.len() > 64
        || !raw
            .session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || raw.seq == 0
        || raw.received_at < 1_577_836_800_000
        || !(-840..=840).contains(&raw.utc_offset_min)
    {
        return Err("Invalid live envelope".into());
    }
    let device = raw
        .fields
        .get(&1)
        .and_then(|n| i64::try_from(*n).ok())
        .and_then(|n| n.checked_mul(1000));
    let ts = device
        .filter(|ts| raw.received_at.abs_diff(*ts) <= 300_000)
        .unwrap_or(raw.received_at);
    let mut out = Vec::new();
    for (field, metric, max, scale) in [
        (2, Metric::Steps, 150_000, 1.0),
        (3, Metric::Distance, 300_000_000, 1000.0),
        (4, Metric::Calories, 20_000, 1.0),
        (7, Metric::HeartRate, 250, 1.0),
    ] {
        let Some(value) = raw.fields.get(&field).copied() else {
            continue;
        };
        if value > max || (metric == Metric::HeartRate && value < 25) {
            continue;
        }
        let mut m = Measurement {
            id: String::new(),
            metric,
            kind: if metric == Metric::HeartRate {
                SampleKind::Instant
            } else {
                SampleKind::CumulativeCounter
            },
            start: ts,
            end: None,
            utc_offset_min: Some(raw.utc_offset_min),
            day: None,
            value: value as f64 / scale,
            unit: metric.canonical_unit(),
            detail: if metric == Metric::Calories {
                Some(Detail::Calories {
                    scope: CalorieScope::Unspecified,
                })
            } else {
                None
            },
            prov: Provenance {
                origin: Origin::BluetoothLive,
                status: DataStatus::Measured,
                source_name: format!("Fitbit Air direct Bluetooth · {}:{}", raw.session, raw.segment),
                device: Some("Fitbit Air".into()),
                original_id: Some(format!("{}:{}:{}", raw.session, raw.seq, field)),
                batch_id: None,
                ingested_at: raw.received_at,
                provisional: true,
                coverage: None,
                confidence: None,
                calc_version: None,
                demo: false,
            },
            superseded_by: None,
        };
        m.id = measurement_id(&m);
        out.push(m);
    }
    Ok(out)
}
