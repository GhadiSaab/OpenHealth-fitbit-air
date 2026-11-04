//! A year of per-minute heart rate (≈525,000 samples) must summarize and
//! downsample quickly, and charts must receive a bounded number of points.

use airspace_core::aggregate::{SummaryContext, summarize};
use airspace_core::dedupe::measurement_id;
use airspace_core::series::{Point, bucketize, choose_width};
use airspace_core::*;
use std::time::Instant;

#[test]
fn year_of_minute_heart_rate() {
    let start = 1_767_225_600_000i64; // 2026-01-01T00:00Z
    let n = 365 * 1440;
    let prov = Provenance {
        origin: Origin::HistoryImport,
        status: DataStatus::Imported,
        source_name: "perf".into(),
        device: None,
        original_id: None,
        batch_id: None,
        ingested_at: 0,
        provisional: false,
        coverage: None,
        confidence: None,
        calc_version: None,
        demo: false,
    };
    let items: Vec<Measurement> = (0..n)
        .map(|i| {
            let mut m = Measurement {
                id: String::new(),
                metric: Metric::HeartRate,
                kind: SampleKind::Instant,
                start: start + i as i64 * 60_000,
                end: None,
                utc_offset_min: None,
                day: None,
                value: 60.0 + (i % 40) as f64,
                unit: Unit::Bpm,
                detail: None,
                prov: prov.clone(),
                superseded_by: None,
            };
            m.id = measurement_id(&m);
            m
        })
        .collect();
    let t = Instant::now();
    let sums = summarize(&items, &SummaryContext { tz: time::parse_tz("Europe/Berlin"), now: i64::MAX / 2, demo: false });
    let summarize_ms = t.elapsed().as_millis();
    assert!((365..=367).contains(&sums.len()), "{}", sums.len());

    let pts: Vec<Point> = items.iter().map(|m| Point { t: m.start, v: m.value }).collect();
    let t = Instant::now();
    let to = start + n as i64 * 60_000;
    let b = bucketize(&pts, start, to, choose_width(start, to, 300, Some(60_000)));
    let bucket_ms = t.elapsed().as_millis();
    assert!(b.len() <= 400, "{}", b.len());
    eprintln!("summarize {n} samples: {summarize_ms} ms; downsample to {} buckets: {bucket_ms} ms", b.len());
    // Generous bounds so the test is stable in debug builds on slow CI.
    assert!(summarize_ms < 30_000 && bucket_ms < 5_000);
}
