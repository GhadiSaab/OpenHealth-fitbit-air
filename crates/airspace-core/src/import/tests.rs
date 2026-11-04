use super::*;
use crate::dedupe::{ConflictPolicy, plan_merge};
use crate::model::{Detail, Metric, SampleKind};

fn ctx() -> ImportContext {
    ImportContext { default_tz: "Europe/Paris".parse().unwrap(), now: 1_790_000_000_000, batch_id: "b1".into(), file_name: "test.json".into() }
}

const SAMPLE: &str = r#"{
  "schema": "airspace.import",
  "version": 1,
  "source": {"name": "Research export", "device": "Fitbit Air"},
  "timezone": "Europe/Paris",
  "records": [
    {"metric": "heart_rate", "time": "2026-06-01T08:00:00+02:00", "value": 62, "unit": "bpm", "id": "hr-1"},
    {"metric": "heart_rate", "time": "2026-06-01T08:01:00", "value": 64},
    {"metric": "heart_rate", "time": "2026-06-01T08:02:00+02:00", "value": 400},
    {"metric": "steps", "kind": "interval", "start": "2026-06-01T08:00:00+02:00", "end": "2026-06-01T08:15:00+02:00", "value": 900},
    {"metric": "steps", "kind": "daily_total", "day": "2026-06-01", "value": 9100},
    {"metric": "distance", "kind": "daily_total", "day": "2026-06-01", "value": 6.4, "unit": "km"},
    {"metric": "skin_temperature", "time": "2026-06-01T07:00:00+02:00", "value": 36.5, "unit": "°C"},
    {"metric": "hrv", "time": "2026-06-01T07:00:00+02:00", "value": 41, "method": "rmssd"},
    {"metric": "sleep", "start": "2026-05-31T23:30:00+02:00", "end": "2026-06-01T07:00:00+02:00",
     "stages": [
        {"stage": "light", "start": "2026-05-31T23:40:00+02:00", "end": "2026-06-01T01:00:00+02:00"},
        {"stage": "deep",  "start": "2026-06-01T01:00:00+02:00", "end": "2026-06-01T02:00:00+02:00"},
        {"stage": "rem",   "start": "2026-06-01T02:00:00+02:00", "end": "2026-06-01T07:00:00+02:00"}
     ]},
    {"metric": "made_up", "value": 1}
  ],
  "journal": [{"day": "2026-06-01", "note": "Ran 5k", "tags": ["Exercise", "  caffeine "]}]
}"#;

#[test]
fn parses_and_validates_json() {
    let p = parse_file("x.json", SAMPLE, &ctx());
    assert_eq!(p.adapter, "airspace-json");
    // 3 rejected: HR 400, absolute skin temperature, unknown metric
    assert_eq!(p.errors(), 3, "{:#?}", p.issues);
    assert_eq!(p.records.len(), 7);
    let dist = p.records.iter().find(|m| m.metric == Metric::Distance).unwrap();
    assert_eq!(dist.value, 6400.0);
    let sleep = p.records.iter().find(|m| m.metric == Metric::Sleep).unwrap();
    assert_eq!(sleep.value, 440.0); // derived from stages
    assert!(matches!(&sleep.detail, Some(Detail::Sleep(d)) if d.asleep_minutes_derived && d.stages.len() == 3));
    assert_eq!(p.journal[0].tags, vec!["exercise", "caffeine"]);
    // Offset-less local time resolved in the file's zone
    let hr2 = p.records.iter().filter(|m| m.metric == Metric::HeartRate).nth(1).unwrap();
    assert_eq!(hr2.start - p.records[0].start, 60_000);
}

#[test]
fn reimport_is_all_duplicates() {
    let p = parse_file("x.json", SAMPLE, &ctx());
    let mut c2 = ctx();
    c2.batch_id = "b2".into();
    c2.now += 1000;
    let p2 = parse_file("x.json", SAMPLE, &c2);
    let pv = preview(&p2, &p.records, &c2.default_tz);
    assert_eq!(pv.totals().new, 0);
    assert_eq!(pv.totals().duplicates, p2.records.len());
}

#[test]
fn conflicting_value_is_detected_and_never_overwritten() {
    let p = parse_file("x.json", SAMPLE, &ctx());
    let changed = SAMPLE.replace("\"value\": 9100", "\"value\": 9300");
    let p2 = parse_file("x.json", &changed, &ctx());
    let pv = preview(&p2, &p.records, &ctx().default_tz);
    assert_eq!(pv.totals().conflicts, 1);
    let keep = plan_merge(p2.records.clone(), &pv.classes, ConflictPolicy::KeepExisting);
    assert_eq!(keep.insert.len(), 0);
    let prefer = plan_merge(p2.records.clone(), &pv.classes, ConflictPolicy::PreferIncoming);
    assert_eq!(prefer.insert.len(), 1);
    assert_eq!(prefer.supersede.len(), 1);
}

#[test]
fn duplicate_within_one_file() {
    let s = r#"{"schema":"airspace.import","version":1,"records":[
      {"metric":"resting_heart_rate","time":"2026-06-01T07:00:00Z","value":55},
      {"metric":"resting_heart_rate","time":"2026-06-01T07:00:00Z","value":55}]}"#;
    let p = parse_file("a.json", s, &ctx());
    let pv = preview(&p, &[], &ctx().default_tz);
    assert_eq!(pv.totals().new, 1);
    assert_eq!(pv.totals().duplicates, 1);
}

#[test]
fn csv_import() {
    let csv = "metric,kind,time,start,end,day,value,unit,method,extra\n\
               heart_rate,,2026-06-01T08:00:00Z,,,,61,bpm,,x\n\
               steps,cumulative_counter,2026-06-01T09:00:00Z,,,,1500,,,\n\
               hrv,,2026-06-01T07:00:00Z,,,,40,ms,rmssd,\n\
               spo2,,2026-06-01T07:00:00Z,,,,abc,%,,\n";
    let p = parse_file("data.csv", csv, &ctx());
    assert_eq!(p.adapter, "airspace-csv");
    assert_eq!(p.records.len(), 3);
    assert_eq!(p.errors(), 1);
    assert!(p.warnings() >= 1); // unknown column
    assert_eq!(p.records[1].kind, SampleKind::CumulativeCounter);
}

#[test]
fn rejects_unknown_format_and_future_dates() {
    let p = parse_file("x.txt", "hello", &ctx());
    assert_eq!(p.errors(), 1);
    let s = r#"{"schema":"airspace.import","version":1,"records":[{"metric":"steps","kind":"daily_total","day":"2099-01-01","value":5}]}"#;
    assert_eq!(parse_file("x.json", s, &ctx()).errors(), 1);
    let s2 = r#"{"schema":"airspace.import","version":2,"records":[]}"#;
    assert_eq!(parse_file("x.json", s2, &ctx()).errors(), 1);
}

#[test]
fn dst_ambiguous_time_warns() {
    let s = r#"{"schema":"airspace.import","version":1,"timezone":"America/New_York","records":[
      {"metric":"heart_rate","time":"2025-11-02T01:30:00","value":60}]}"#;
    let p = parse_file("x.json", s, &ctx());
    assert_eq!(p.records.len(), 1);
    assert_eq!(p.warnings(), 1);
}
