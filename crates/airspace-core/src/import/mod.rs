//! History import: adapter boundary, validation and preview.
//!
//! An [`ImportAdapter`] turns a file into [`RawRecord`]s (or directly into
//! measurements). All adapters share the same validation in
//! [`raw::to_measurement`], so every import path enforces identical rules.
//!
//! Adapters in this build:
//! * `airspace-json` — the documented Airspace import schema (docs/IMPORT_SCHEMA.md)
//! * `airspace-csv`  — the same fields as CSV columns
//!
//! Existing research exports are *not* assumed to match this schema; convert
//! them with a small script or add an adapter (see docs/IMPORT_SCHEMA.md).

pub mod csv;
pub mod json;
pub mod raw;

use crate::dedupe::{Classification, classify};
use crate::journal::JournalEntry;
use crate::model::{Measurement, Metric};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use raw::RawRecord;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The record was skipped.
    Error,
    /// The record was kept but something is worth knowing.
    Warning,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Issue {
    /// 1-based record (or CSV line) number; 0 for file-level problems.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct ImportContext {
    /// Zone for timestamps without an offset (from the file, else the user's).
    pub default_tz: Tz,
    pub now: i64,
    pub batch_id: String,
    pub file_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct ParsedImport {
    pub adapter: String,
    pub source_name: String,
    pub records: Vec<Measurement>,
    pub journal: Vec<JournalEntry>,
    pub issues: Vec<Issue>,
}

impl ParsedImport {
    pub fn errors(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == Severity::Error).count()
    }
    pub fn warnings(&self) -> usize {
        self.issues.iter().filter(|i| i.severity == Severity::Warning).count()
    }
}

/// The adapter boundary. Implement this to support a new history format.
pub trait ImportAdapter {
    /// Stable identifier, recorded with every imported measurement's batch.
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    /// Cheap check of whether this adapter recognizes the file.
    fn detect(&self, file_name: &str, content: &str) -> bool;
    fn parse(&self, content: &str, ctx: &ImportContext) -> ParsedImport;
}

pub fn adapters() -> Vec<Box<dyn ImportAdapter>> {
    vec![Box::new(json::AirspaceJson), Box::new(csv::AirspaceCsv)]
}

/// Picks an adapter for a file and parses it.
pub fn parse_file(file_name: &str, content: &str, ctx: &ImportContext) -> ParsedImport {
    if content.len() > 200 * 1024 * 1024 {
        return ParsedImport {
            issues: vec![Issue { line: 0, severity: Severity::Error, message: "File is larger than 200 MB.".into() }],
            ..Default::default()
        };
    }
    for a in adapters() {
        if a.detect(file_name, content) {
            return a.parse(content, ctx);
        }
    }
    ParsedImport {
        issues: vec![Issue {
            line: 0,
            severity: Severity::Error,
            message: "This file isn't in a recognized format. Airspace accepts its JSON import schema (\"schema\": \"airspace.import\"), its CSV columns, or an Airspace backup. See the import guide.".into(),
        }],
        ..Default::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MetricCounts {
    pub new: usize,
    pub duplicates: usize,
    pub conflicts: usize,
    pub first: Option<i64>,
    pub last: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct ImportPreview {
    pub per_metric: BTreeMap<Metric, MetricCounts>,
    pub classes: Vec<Classification>,
    pub first_day: Option<String>,
    pub last_day: Option<String>,
    pub days: usize,
}

impl ImportPreview {
    pub fn totals(&self) -> MetricCounts {
        let mut t = MetricCounts::default();
        for c in self.per_metric.values() {
            t.new += c.new;
            t.duplicates += c.duplicates;
            t.conflicts += c.conflicts;
        }
        t
    }
}

/// Classifies parsed records against the existing ones that overlap them.
pub fn preview(parsed: &ParsedImport, existing: &[Measurement], tz: &Tz) -> ImportPreview {
    let classes = classify(&parsed.records, existing);
    let mut per: BTreeMap<Metric, MetricCounts> = BTreeMap::new();
    let mut days = std::collections::BTreeSet::new();
    for (m, c) in parsed.records.iter().zip(&classes) {
        let e = per.entry(m.metric).or_default();
        match c {
            Classification::New => e.new += 1,
            Classification::Duplicate => e.duplicates += 1,
            Classification::Conflict { .. } => e.conflicts += 1,
        }
        e.first = Some(e.first.map_or(m.start, |f| f.min(m.start)));
        e.last = Some(e.last.map_or(m.end_or_start(), |f| f.max(m.end_or_start())));
        days.insert(crate::aggregate::day_of(m, tz));
    }
    ImportPreview {
        per_metric: per,
        classes,
        first_day: days.first().map(|d| crate::time::day_key(*d)),
        last_day: days.last().map(|d| crate::time::day_key(*d)),
        days: days.len(),
    }
}

/// Time span covered by parsed records, for loading overlapping existing data.
pub fn span(records: &[Measurement]) -> Option<(i64, i64)> {
    let a = records.iter().map(|m| m.start).min()?;
    let b = records.iter().map(|m| m.end_or_start()).max()?;
    Some((a, b + 1))
}

#[cfg(test)]
mod tests;
