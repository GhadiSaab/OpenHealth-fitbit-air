//! Versioned JSON backup/restore and per-metric CSV export.
//!
//! Backups contain measurements (with full provenance), journal entries,
//! import batches and non-secret settings. They never contain AI API keys:
//! [`Settings`] has no field that could hold one, and AI configuration is
//! exported without its key (see [`crate::ai::AiConfig`]).
//! Format: docs/BACKUP_FORMAT.md.

use crate::ai::AiConfig;
use crate::import::csv::escape;
use crate::journal::JournalEntry;
use crate::model::{DbKind, ImportBatch, Measurement, Metric};
use crate::settings::Settings;
use crate::time;
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

pub const BACKUP_FORMAT: &str = "airspace.backup";
pub const BACKUP_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub created_at: i64,
    pub app_version: String,
    pub database: DbKind,
    pub settings: Settings,
    /// AI provider configuration without any credential.
    #[serde(default)]
    pub ai: Option<AiConfig>,
    pub measurements: Vec<Measurement>,
    pub journal: Vec<JournalEntry>,
    pub batches: Vec<ImportBatch>,
}

impl Backup {
    pub fn new(
        db: DbKind,
        now: i64,
        settings: Settings,
        ai: Option<AiConfig>,
        measurements: Vec<Measurement>,
        journal: Vec<JournalEntry>,
        batches: Vec<ImportBatch>,
    ) -> Self {
        Backup {
            format: BACKUP_FORMAT.into(),
            version: BACKUP_VERSION,
            created_at: now,
            app_version: env!("CARGO_PKG_VERSION").into(),
            database: db,
            settings,
            ai,
            measurements,
            journal,
            batches,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RestoreError {
    #[error("This is not an Airspace backup file.")]
    NotABackup,
    #[error("Backup version {0} was made by a newer version of Airspace.")]
    TooNew(u32),
    #[error("The backup file is damaged: {0}")]
    Malformed(String),
}

pub fn is_backup(content: &str) -> bool {
    let head: String = content.chars().take(512).collect();
    head.contains(BACKUP_FORMAT)
}

/// Parses and validates a backup, migrating older versions.
pub fn parse_backup(content: &str) -> Result<Backup, RestoreError> {
    let v: serde_json::Value = serde_json::from_str(content).map_err(|e| RestoreError::Malformed(e.to_string()))?;
    if v.get("format").and_then(|f| f.as_str()) != Some(BACKUP_FORMAT) {
        return Err(RestoreError::NotABackup);
    }
    let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    if ver > BACKUP_VERSION {
        return Err(RestoreError::TooNew(ver));
    }
    // Version 1 is current. Future migrations transform `v` here, step by step.
    let b: Backup = serde_json::from_value(v).map_err(|e| RestoreError::Malformed(e.to_string()))?;
    for m in &b.measurements {
        if m.id.is_empty() {
            return Err(RestoreError::Malformed("a measurement has no id".into()));
        }
    }
    Ok(b)
}

/// The serde (snake_case) name of an enum value, as used in JSON.
fn serde_name<T: Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

/// CSV for one metric. Columns are documented in docs/BACKUP_FORMAT.md.
pub fn metric_csv(metric: Metric, items: &[Measurement], tz: &Tz) -> String {
    let mut out = String::from(
        "id,metric,kind,start_utc,end_utc,local_day,utc_offset_min,value,unit,origin,status,source,device,original_id,batch_id,ingested_utc,provisional,confidence,coverage,calc_version,superseded_by,detail\n",
    );
    let iso = |ms: i64| time::format_ts(ms, &Tz::UTC, "%Y-%m-%dT%H:%M:%S%.3fZ");
    let mut rows: Vec<&Measurement> = items.iter().filter(|m| m.metric == metric).collect();
    rows.sort_by_key(|m| m.start);
    for m in rows {
        let detail = m.detail.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default()).unwrap_or_default();
        let fields = [
            m.id.clone(),
            m.metric.key().into(),
            serde_name(&m.kind),
            iso(m.start),
            m.end.map(iso).unwrap_or_default(),
            time::day_key(crate::aggregate::day_of(m, tz)),
            m.utc_offset_min.map(|o| o.to_string()).unwrap_or_default(),
            format!("{}", m.value),
            m.unit.key().into(),
            serde_name(&m.prov.origin),
            serde_name(&m.prov.status),
            m.prov.source_name.clone(),
            m.prov.device.clone().unwrap_or_default(),
            m.prov.original_id.clone().unwrap_or_default(),
            m.prov.batch_id.clone().unwrap_or_default(),
            iso(m.prov.ingested_at),
            m.prov.provisional.to_string(),
            m.prov.confidence.map(|c| c.to_string()).unwrap_or_default(),
            m.prov.coverage.map(|c| format!("{c}")).unwrap_or_default(),
            m.prov.calc_version.map(|c| c.to_string()).unwrap_or_default(),
            m.superseded_by.clone().unwrap_or_default(),
            detail,
        ];
        out.push_str(&fields.iter().map(|f| escape(f)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{AiConfig, AiProviderKind};
    use crate::demo;

    #[test]
    fn backup_round_trip_and_contains_no_secrets() {
        let ds = demo::generate(chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(), &Tz::UTC, 1_782_000_000_000);
        let ai = AiConfig { provider: AiProviderKind::Anthropic, endpoint: "https://api.anthropic.com".into(), model: "claude-opus-5-5".into() };
        let b = Backup::new(DbKind::Demo, 0, Settings::default(), Some(ai), ds.measurements.clone(), ds.journal.clone(), vec![]);
        let json = b.to_json();
        let lower = json.to_lowercase();
        assert!(!lower.contains("api_key") && !lower.contains("apikey") && !lower.contains("x-api-key") && !lower.contains("sk-"));
        let back = parse_backup(&json).unwrap();
        assert_eq!(back.measurements.len(), ds.measurements.len());
        assert_eq!(back, b);
    }

    #[test]
    fn rejects_newer_versions() {
        let s = r#"{"format":"airspace.backup","version":99}"#;
        assert_eq!(parse_backup(s), Err(RestoreError::TooNew(99)));
        assert_eq!(parse_backup(r#"{"format":"x"}"#), Err(RestoreError::NotABackup));
    }

    #[test]
    fn csv_export_escapes_and_has_header() {
        let ds = demo::generate(chrono::NaiveDate::from_ymd_opt(2026, 6, 30).unwrap(), &Tz::UTC, 1_782_000_000_000);
        let csv = metric_csv(Metric::Sleep, &ds.measurements, &Tz::UTC);
        let rows = crate::import::csv::parse_rows(&csv);
        assert!(rows.len() > 20);
        assert_eq!(rows[0][0], "id");
        assert!(rows.iter().skip(1).all(|r| r.len() == rows[0].len()));
        let steps = metric_csv(Metric::Steps, &ds.measurements, &Tz::UTC);
        assert!(steps.contains(",cumulative_counter,") && steps.contains(",bluetooth_live,") && steps.contains(",history_import,"));
    }
}
