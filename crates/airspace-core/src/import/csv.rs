//! CSV import using the same field names as the JSON schema.
//!
//! The first line is a header. Required columns: `metric`, `value` (except
//! sleep sessions), and the time fields the record kind needs. Unknown
//! columns are ignored with a warning. Sleep stages cannot be expressed in
//! CSV; use JSON for staged sleep.

use super::raw::{RawRecord, SourceInfo, convert_all};
use super::{ImportAdapter, ImportContext, Issue, ParsedImport, Severity};

/// Splits CSV text into rows of fields (RFC 4180 quoting).
pub fn parse_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = vec![];
    let mut row = vec![];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| !(r.len() == 1 && r[0].trim().is_empty()));
    rows
}

/// Escapes one CSV field, and neutralizes spreadsheet formula injection
/// for text that starts with `=`, `+`, `-` or `@`.
pub fn escape(s: &str) -> String {
    let formula = s.starts_with(['=', '+', '-', '@']) && s.parse::<f64>().is_err();
    let s = if formula { format!("'{s}") } else { s.to_string() };
    if formula || s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s }
}

const KNOWN: [&str; 22] = [
    "metric", "kind", "time", "start", "end", "day", "value", "unit", "id", "confidence", "provisional", "method", "scope", "zone",
    "lower_bpm", "upper_bpm", "zone_origin", "stage", "min", "max", "resets_daily", "asleep_minutes",
];

pub struct AirspaceCsv;

impl ImportAdapter for AirspaceCsv {
    fn id(&self) -> &'static str {
        "airspace-csv"
    }
    fn name(&self) -> &'static str {
        "Airspace import (CSV)"
    }
    fn detect(&self, file_name: &str, content: &str) -> bool {
        let first = content.lines().next().unwrap_or("").to_ascii_lowercase();
        (file_name.to_ascii_lowercase().ends_with(".csv") || first.contains(',')) && first.split(',').any(|c| c.trim() == "metric")
    }
    fn parse(&self, content: &str, ctx: &ImportContext) -> ParsedImport {
        let mut out = ParsedImport { adapter: self.id().into(), source_name: format!("CSV import: {}", ctx.file_name), ..Default::default() };
        let rows = parse_rows(content.trim_start_matches('\u{feff}'));
        let Some(header) = rows.first() else {
            out.issues.push(Issue { line: 0, severity: Severity::Error, message: "The CSV file is empty.".into() });
            return out;
        };
        let header: Vec<String> = header.iter().map(|h| h.trim().to_ascii_lowercase()).collect();
        for h in &header {
            if !KNOWN.contains(&h.as_str()) {
                out.issues.push(Issue { line: 1, severity: Severity::Warning, message: format!("Column \"{h}\" is not part of the schema and was ignored.") });
            }
        }
        let col = |name: &str| header.iter().position(|h| h == name);
        let mut raws = vec![];
        for (i, row) in rows.iter().enumerate().skip(1) {
            let line = i + 1;
            let get = |name: &str| col(name).and_then(|c| row.get(c)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
            let num = |name: &str| -> Result<Option<f64>, String> {
                match get(name) {
                    None => Ok(None),
                    Some(v) => v.parse::<f64>().map(Some).map_err(|_| format!("\"{name}\" is not a number (\"{v}\").")),
                }
            };
            let parsed = (|| -> Result<RawRecord, String> {
                Ok(RawRecord {
                    metric: get("metric").ok_or("Missing \"metric\".")?,
                    kind: get("kind"),
                    time: get("time"),
                    start: get("start"),
                    end: get("end"),
                    day: get("day"),
                    value: num("value")?,
                    unit: get("unit"),
                    id: get("id"),
                    confidence: num("confidence")?.map(|v| v as u8),
                    provisional: get("provisional").map(|v| v == "true" || v == "1"),
                    method: get("method"),
                    scope: get("scope"),
                    zone: get("zone"),
                    lower_bpm: num("lower_bpm")?,
                    upper_bpm: num("upper_bpm")?,
                    zone_origin: get("zone_origin"),
                    stage: get("stage"),
                    min: num("min")?,
                    max: num("max")?,
                    resets_daily: get("resets_daily").map(|v| v == "true" || v == "1"),
                    asleep_minutes: num("asleep_minutes")?,
                    main: None,
                    stages: None,
                })
            })();
            match parsed {
                Ok(r) => raws.push((line, r)),
                Err(e) => out.issues.push(Issue { line, severity: Severity::Error, message: e }),
            }
        }
        let src = SourceInfo { name: out.source_name.clone(), device: None, adapter: "airspace-csv", tz: ctx.default_tz };
        convert_all(&raws, &src, ctx, &mut out);
        out
    }
}
