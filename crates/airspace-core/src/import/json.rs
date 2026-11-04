//! Airspace JSON import schema (`"schema": "airspace.import"`, version 1).

use super::raw::{RawRecord, SourceInfo, convert_all};
use super::{ImportAdapter, ImportContext, Issue, ParsedImport, Severity};
use crate::journal::{JournalEntry, normalize_tag};
use crate::time;
use serde::Deserialize;

pub const SCHEMA: &str = "airspace.import";
pub const VERSION: u32 = 1;

#[derive(Deserialize)]
struct FileSource {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    device: Option<String>,
}

#[derive(Deserialize)]
struct RawJournal {
    day: String,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct ImportFile {
    schema: String,
    version: u32,
    #[serde(default)]
    source: Option<FileSource>,
    #[serde(default)]
    timezone: Option<String>,
    #[serde(default)]
    records: Vec<serde_json::Value>,
    #[serde(default)]
    journal: Vec<RawJournal>,
}

pub struct AirspaceJson;

impl ImportAdapter for AirspaceJson {
    fn id(&self) -> &'static str {
        "airspace-json"
    }
    fn name(&self) -> &'static str {
        "Airspace import (JSON)"
    }
    fn detect(&self, _file_name: &str, content: &str) -> bool {
        let head: String = content.chars().take(4096).collect();
        head.trim_start().starts_with('{') && head.contains(SCHEMA)
    }
    fn parse(&self, content: &str, ctx: &ImportContext) -> ParsedImport {
        let mut out = ParsedImport { adapter: self.id().into(), ..Default::default() };
        let file: ImportFile = match serde_json::from_str(content) {
            Ok(f) => f,
            Err(e) => {
                out.issues.push(Issue { line: 0, severity: Severity::Error, message: format!("The file is not valid JSON for the import schema: {e}") });
                return out;
            }
        };
        if file.schema != SCHEMA {
            out.issues.push(Issue { line: 0, severity: Severity::Error, message: format!("Unexpected schema \"{}\".", file.schema) });
            return out;
        }
        if file.version != VERSION {
            out.issues.push(Issue {
                line: 0,
                severity: Severity::Error,
                message: format!("Import schema version {} is not supported (this app reads version {VERSION}).", file.version),
            });
            return out;
        }
        let tz = match &file.timezone {
            Some(t) => match t.parse() {
                Ok(z) => z,
                Err(_) => {
                    out.issues.push(Issue { line: 0, severity: Severity::Warning, message: format!("Unknown time zone \"{t}\"; using your home time zone.") });
                    ctx.default_tz
                }
            },
            None => ctx.default_tz,
        };
        let src = SourceInfo {
            name: file.source.as_ref().and_then(|s| s.name.clone()).unwrap_or_else(|| format!("Import: {}", ctx.file_name)),
            device: file.source.as_ref().and_then(|s| s.device.clone()),
            adapter: "airspace-json",
            tz,
        };
        out.source_name = src.name.clone();
        let mut raws = Vec::with_capacity(file.records.len());
        for (i, v) in file.records.into_iter().enumerate() {
            match serde_json::from_value::<RawRecord>(v) {
                Ok(r) => raws.push((i + 1, r)),
                Err(e) => out.issues.push(Issue { line: i + 1, severity: Severity::Error, message: format!("Record is malformed: {e}") }),
            }
        }
        convert_all(&raws, &src, ctx, &mut out);
        for (i, j) in file.journal.into_iter().enumerate() {
            let Some(d) = time::parse_day(&j.day) else {
                out.issues.push(Issue { line: i + 1, severity: Severity::Error, message: format!("Journal entry {} has an invalid day.", i + 1) });
                continue;
            };
            let mut e = JournalEntry::new(d, ctx.now);
            e.note = j.note.unwrap_or_default();
            e.tags = j.tags.iter().filter_map(|t| normalize_tag(t)).collect();
            out.journal.push(e);
        }
        out
    }
}
