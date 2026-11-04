//! Identity, duplicate detection and conflict classification.
//!
//! * A **duplicate** is a record whose deterministic id already exists: the
//!   same metric, time, value and source. Duplicates are skipped.
//! * A **conflict** is a record for the same metric, kind, time span and
//!   detail discriminant but a *different* value. Conflicts are resolved by an
//!   explicit [`ConflictPolicy`]; originals are never overwritten.

use crate::model::{Measurement, Origin, SampleKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// FNV-1a 64-bit. Small, deterministic and identical on every platform.
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn origin_tag(o: Origin) -> &'static str {
    match o {
        Origin::HistoryImport => "imp",
        Origin::BluetoothLive => "ble",
        Origin::Derived => "der",
    }
}

/// Deterministic id for a measurement's content.
pub fn measurement_id(m: &Measurement) -> String {
    // Values are rounded to 1e-6 so float formatting noise does not create
    // distinct ids for the same reading.
    let value = (m.value * 1e6).round() as i64;
    let key = format!(
        "{}|{:?}|{}|{}|{}|{}|{}|{}|{}",
        m.metric.key(),
        m.kind,
        m.start,
        m.end.unwrap_or(-1),
        m.day.as_deref().unwrap_or(""),
        value,
        m.detail.as_ref().map(|d| d.discriminant()).unwrap_or_default(),
        m.prov.source_name,
        m.prov.original_id.as_deref().unwrap_or(""),
    );
    format!("{}-{:016x}", origin_tag(m.prov.origin), fnv1a(key.as_bytes()))
}

/// Assigns ids to records that lack one.
pub fn ensure_ids(items: &mut [Measurement]) {
    for m in items.iter_mut() {
        if m.id.is_empty() {
            m.id = measurement_id(m);
        }
    }
}

/// Key identifying "the same observation slot" regardless of value.
pub fn conflict_key(m: &Measurement) -> String {
    let span = match m.kind {
        SampleKind::DailyTotal => m.day.clone().unwrap_or_default(),
        _ => format!("{}-{}", m.start, m.end.unwrap_or(-1)),
    };
    format!(
        "{}|{:?}|{}|{}",
        m.metric.key(),
        m.kind,
        span,
        m.detail.as_ref().map(|d| d.discriminant()).unwrap_or_default()
    )
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    /// Keep the stored record; do not add the incoming one.
    KeepExisting,
    /// Store both. Aggregation then follows source precedence rules.
    KeepBoth,
    /// Store the incoming record and mark the stored one as superseded
    /// (it remains in the database and in backups).
    PreferIncoming,
}

impl ConflictPolicy {
    pub fn key(self) -> &'static str {
        match self {
            ConflictPolicy::KeepExisting => "keep_existing",
            ConflictPolicy::KeepBoth => "keep_both",
            ConflictPolicy::PreferIncoming => "prefer_incoming",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Classification {
    New,
    Duplicate,
    /// Conflicts with the stored record with this id.
    Conflict { existing_id: String, existing_value: f64 },
}

/// Classifies `incoming` records against `existing` ones (and against each
/// other — a file may contain the same reading twice).
pub fn classify(incoming: &[Measurement], existing: &[Measurement]) -> Vec<Classification> {
    let mut ids: HashMap<&str, ()> = existing.iter().map(|m| (m.id.as_str(), ())).collect();
    let mut slots: HashMap<String, (String, f64)> = existing
        .iter()
        .filter(|m| m.is_active())
        .map(|m| (conflict_key(m), (m.id.clone(), m.value)))
        .collect();
    let mut out = Vec::with_capacity(incoming.len());
    for m in incoming {
        if ids.contains_key(m.id.as_str()) {
            out.push(Classification::Duplicate);
            continue;
        }
        ids.insert(m.id.as_str(), ());
        let key = conflict_key(m);
        match slots.get(&key) {
            Some((eid, ev)) if (ev - m.value).abs() > 1e-9 => out.push(Classification::Conflict {
                existing_id: eid.clone(),
                existing_value: *ev,
            }),
            Some(_) => out.push(Classification::Duplicate),
            None => {
                slots.insert(key, (m.id.clone(), m.value));
                out.push(Classification::New)
            }
        }
    }
    out
}

/// The writes needed to apply a classified import under `policy`.
#[derive(Debug, Default, Clone)]
pub struct MergePlan {
    pub insert: Vec<Measurement>,
    /// Existing ids to mark as superseded by the given incoming id.
    pub supersede: Vec<(String, String)>,
    pub skipped_duplicates: usize,
    pub skipped_conflicts: usize,
    pub kept_both: usize,
}

pub fn plan_merge(incoming: Vec<Measurement>, classes: &[Classification], policy: ConflictPolicy) -> MergePlan {
    let mut plan = MergePlan::default();
    for (m, c) in incoming.into_iter().zip(classes) {
        match c {
            Classification::New => plan.insert.push(m),
            Classification::Duplicate => plan.skipped_duplicates += 1,
            Classification::Conflict { existing_id, .. } => match policy {
                ConflictPolicy::KeepExisting => plan.skipped_conflicts += 1,
                ConflictPolicy::KeepBoth => {
                    plan.kept_both += 1;
                    plan.insert.push(m)
                }
                ConflictPolicy::PreferIncoming => {
                    plan.supersede.push((existing_id.clone(), m.id.clone()));
                    plan.insert.push(m)
                }
            },
        }
    }
    plan
}
