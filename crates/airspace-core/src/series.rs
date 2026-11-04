//! Chart-ready series: gap detection and downsampling.
//!
//! Original samples are never altered; these functions produce display
//! series only.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub t: i64,
    pub v: f64,
}

/// One aggregated bucket of samples.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    pub t0: i64,
    pub t1: i64,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub n: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    pub start: i64,
    pub end: i64,
}

/// Median spacing between consecutive points.
pub fn median_interval(points: &[Point]) -> Option<i64> {
    if points.len() < 2 {
        return None;
    }
    let mut d: Vec<i64> = points.windows(2).map(|w| w[1].t - w[0].t).filter(|d| *d > 0).collect();
    if d.is_empty() {
        return None;
    }
    d.sort_unstable();
    Some(d[d.len() / 2])
}

/// The gap threshold for a series: three times its typical spacing, but at
/// least `min_gap` ms.
pub fn gap_threshold(points: &[Point], min_gap: i64) -> i64 {
    median_interval(points).map(|m| (m * 3).max(min_gap)).unwrap_or(min_gap)
}

/// Splits a time-sorted series at gaps larger than `threshold`, so lines are
/// never drawn (interpolated) across missing data.
pub fn segments(points: &[Point], threshold: i64) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = vec![];
    for p in points {
        match out.last_mut() {
            Some(seg) if p.t - seg.last().unwrap().t <= threshold => seg.push(*p),
            _ => out.push(vec![*p]),
        }
    }
    out
}

/// Recording gaps within `[from, to)`: spans with no samples longer than
/// `threshold`, including at the edges.
pub fn gaps(points: &[Point], from: i64, to: i64, threshold: i64) -> Vec<Gap> {
    let mut out = vec![];
    let mut prev = from;
    for p in points.iter().filter(|p| p.t >= from && p.t < to) {
        if p.t - prev > threshold {
            out.push(Gap { start: prev, end: p.t });
        }
        prev = p.t;
    }
    if to - prev > threshold {
        out.push(Gap { start: prev, end: to });
    }
    out
}

/// Aggregates points into fixed-width time buckets (min/max/avg). Empty
/// buckets are omitted — they are gaps, not zeros.
pub fn bucketize(points: &[Point], from: i64, to: i64, width: i64) -> Vec<Bucket> {
    if width <= 0 || to <= from {
        return vec![];
    }
    let n = ((to - from) + width - 1) / width;
    let mut acc: Vec<Option<(f64, f64, f64, u32)>> = vec![None; n as usize];
    for p in points {
        if p.t < from || p.t >= to {
            continue;
        }
        let i = ((p.t - from) / width) as usize;
        let e = acc[i].get_or_insert((f64::INFINITY, f64::NEG_INFINITY, 0.0, 0));
        e.0 = e.0.min(p.v);
        e.1 = e.1.max(p.v);
        e.2 += p.v;
        e.3 += 1;
    }
    acc.into_iter()
        .enumerate()
        .filter_map(|(i, a)| {
            a.map(|(mn, mx, sum, c)| Bucket {
                t0: from + i as i64 * width,
                t1: (from + (i as i64 + 1) * width).min(to),
                min: mn,
                max: mx,
                avg: sum / c as f64,
                n: c,
            })
        })
        .collect()
}

/// Sums interval values into buckets (for steps per hour, etc.).
pub fn bucket_sums(points: &[Point], from: i64, to: i64, width: i64) -> Vec<Bucket> {
    bucketize(points, from, to, width)
        .into_iter()
        .map(|b| Bucket { avg: b.avg * b.n as f64, ..b })
        .collect()
}

/// Chooses a bucket width so roughly `target` buckets cover `[from, to)`,
/// snapped to a friendly duration, and never finer than the data.
pub fn choose_width(from: i64, to: i64, target: i64, data_interval: Option<i64>) -> i64 {
    const STEPS: [i64; 15] = [
        60_000, 120_000, 300_000, 600_000, 900_000, 1_800_000, 3_600_000, 7_200_000, 10_800_000, 21_600_000, 43_200_000, 86_400_000,
        172_800_000, 604_800_000, 2_419_200_000,
    ];
    let raw = ((to - from) / target.max(1)).max(1);
    let min = data_interval.unwrap_or(0);
    STEPS.into_iter().find(|s| *s >= raw && *s >= min).unwrap_or(*STEPS.last().unwrap())
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Stats {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub sum: f64,
    pub n: usize,
}

pub fn stats(values: impl IntoIterator<Item = f64>) -> Stats {
    let mut s = Stats::default();
    for v in values {
        s.min = Some(s.min.map_or(v, |m: f64| m.min(v)));
        s.max = Some(s.max.map_or(v, |m: f64| m.max(v)));
        s.sum += v;
        s.n += 1;
    }
    if s.n > 0 {
        s.mean = Some(s.sum / s.n as f64);
    }
    s
}

pub fn std_dev(values: &[f64]) -> Option<f64> {
    if values.len() < 2 {
        return None;
    }
    let m = values.iter().sum::<f64>() / values.len() as f64;
    Some((values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64).sqrt())
}

/// Trailing rolling mean over `window` entries, requiring at least
/// `min_points` non-missing values in the window.
pub fn rolling_mean(values: &[Option<f64>], window: usize, min_points: usize) -> Vec<Option<f64>> {
    (0..values.len())
        .map(|i| {
            let lo = (i + 1).saturating_sub(window);
            let w: Vec<f64> = values[lo..=i].iter().flatten().copied().collect();
            if w.len() >= min_points { Some(w.iter().sum::<f64>() / w.len() as f64) } else { None }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(ts: &[i64]) -> Vec<Point> {
        ts.iter().map(|t| Point { t: *t, v: 1.0 }).collect()
    }

    #[test]
    fn segments_split_at_gaps() {
        let p = pts(&[0, 60, 120, 1000, 1060]);
        let s = segments(&p, 180);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].len(), 2);
    }

    #[test]
    fn gaps_include_edges() {
        let p = pts(&[500, 560]);
        let g = gaps(&p, 0, 2000, 200);
        assert_eq!(g, vec![Gap { start: 0, end: 500 }, Gap { start: 560, end: 2000 }]);
    }

    #[test]
    fn empty_buckets_are_omitted_not_zero() {
        let p = vec![Point { t: 10, v: 60.0 }, Point { t: 20, v: 80.0 }, Point { t: 350, v: 100.0 }];
        let b = bucketize(&p, 0, 400, 100);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].avg, 70.0);
        assert_eq!(b[1].t0, 300);
    }

    #[test]
    fn rolling_requires_min_points() {
        let v = vec![Some(1.0), None, None, Some(3.0)];
        let r = rolling_mean(&v, 3, 2);
        assert_eq!(r, vec![None, None, None, None]);
        let r = rolling_mean(&v, 4, 2);
        assert_eq!(r[3], Some(2.0));
    }

    #[test]
    fn large_dataset_downsamples() {
        let p: Vec<Point> = (0..100_000).map(|i| Point { t: i * 1000, v: (i % 100) as f64 }).collect();
        let b = bucketize(&p, 0, 100_000_000, choose_width(0, 100_000_000, 300, Some(1000)));
        assert!(b.len() <= 400);
    }
}
