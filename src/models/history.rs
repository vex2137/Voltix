use std::collections::VecDeque;
use std::fmt::Write;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HistorySample {
    pub timestamp: Instant,
    pub value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChartPaths {
    pub line_commands: String,
    pub area_commands: String,
    pub has_data: bool,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub latest_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct BoundedHistory {
    capacity: usize,
    samples: VecDeque<HistorySample>,
}

impl BoundedHistory {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(2);
        Self {
            capacity: cap,
            samples: VecDeque::with_capacity(cap),
        }
    }

    pub fn push(&mut self, timestamp: Instant, value: Option<f64>) {
        if self.samples.len() >= self.capacity {
            self.samples.pop_front();
        }
        self.samples.push_back(HistorySample { timestamp, value });
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn samples(&self) -> &VecDeque<HistorySample> {
        &self.samples
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_chart_paths(
        &self,
        now: Instant,
        window: Duration,
        width: f64,
        height: f64,
        min_y: f64,
        max_y: f64,
        max_gap: Duration,
    ) -> ChartPaths {
        let window_secs = window.as_secs_f64();
        if window_secs <= 0.0 || width <= 0.0 || height <= 0.0 || max_y <= min_y {
            return ChartPaths {
                line_commands: String::new(),
                area_commands: String::new(),
                has_data: false,
                min_value: None,
                max_value: None,
                latest_value: None,
            };
        }

        let cutoff = now.checked_sub(window);
        let max_gap_secs = max_gap.as_secs_f64();
        let y_span = max_y - min_y;

        let mut line_commands = String::with_capacity(self.samples.len() * 18);
        let mut area_commands = String::with_capacity(self.samples.len() * 20);

        let mut segment_points: Vec<(f64, f64)> = Vec::new();
        let mut prev_t: Option<Instant> = None;
        let mut min_seen: Option<f64> = None;
        let mut max_seen: Option<f64> = None;
        let mut latest_seen: Option<f64> = None;
        let mut has_data = false;

        let flush_segment = |pts: &[(f64, f64)], line: &mut String, area: &mut String| {
            if pts.is_empty() {
                return;
            }
            if pts.len() == 1 {
                let (x, y) = pts[0];
                let x2 = (x + 1.5).min(width);
                let _ = write!(line, "M {x:.2} {y:.2} L {x2:.2} {y:.2} ");
                let _ = write!(
                    area,
                    "M {x:.2} {height:.2} L {x:.2} {y:.2} L {x2:.2} {y:.2} L {x2:.2} {height:.2} Z "
                );
                return;
            }
            let (first_x, first_y) = pts[0];
            let _ = write!(line, "M {first_x:.2} {first_y:.2}");
            let _ = write!(area, "M {first_x:.2} {height:.2} L {first_x:.2} {first_y:.2}");
            for &(x, y) in &pts[1..] {
                let _ = write!(line, " L {x:.2} {y:.2}");
                let _ = write!(area, " L {x:.2} {y:.2}");
            }
            if let Some(&(last_x, _)) = pts.last() {
                let _ = write!(area, " L {last_x:.2} {height:.2} Z ");
            }
            line.push(' ');
        };

        for sample in &self.samples {
            if let Some(c) = cutoff {
                if sample.timestamp < c {
                    continue;
                }
            }
            if sample.timestamp > now {
                continue;
            }

            let age = now.saturating_duration_since(sample.timestamp).as_secs_f64();
            let x = ((1.0 - (age / window_secs)).clamp(0.0, 1.0)) * width;

            let is_gap = prev_t
                .map(|pt| {
                    sample
                        .timestamp
                        .saturating_duration_since(pt)
                        .as_secs_f64()
                        > max_gap_secs
                })
                .unwrap_or(false);

            if is_gap {
                flush_segment(&segment_points, &mut line_commands, &mut area_commands);
                segment_points.clear();
            }

            match sample.value {
                Some(v) if v.is_finite() => {
                    has_data = true;
                    min_seen = Some(min_seen.map_or(v, |m| m.min(v)));
                    max_seen = Some(max_seen.map_or(v, |m| m.max(v)));
                    latest_seen = Some(v);

                    let norm = ((v - min_y) / y_span).clamp(0.0, 1.0);
                    let y = height - (norm * height);
                    segment_points.push((x, y));
                    prev_t = Some(sample.timestamp);
                }
                _ => {
                    // Explicitly break the path on missing/unavailable samples so no
                    // misleading interpolation is drawn across gaps.
                    flush_segment(&segment_points, &mut line_commands, &mut area_commands);
                    segment_points.clear();
                    prev_t = None;
                }
            }
        }

        flush_segment(&segment_points, &mut line_commands, &mut area_commands);

        ChartPaths {
            line_commands: line_commands.trim_end().to_string(),
            area_commands: area_commands.trim_end().to_string(),
            has_data,
            min_value: min_seen,
            max_value: max_seen,
            latest_value: latest_seen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_history_evicts_oldest_at_capacity() {
        let t0 = Instant::now();
        let mut hist = BoundedHistory::new(3);
        for i in 0..5 {
            hist.push(t0 + Duration::from_secs(i), Some(i as f64));
        }
        assert_eq!(hist.len(), 3);
        let vals: Vec<Option<f64>> = hist.samples().iter().map(|s| s.value).collect();
        assert_eq!(vals, vec![Some(2.0), Some(3.0), Some(4.0)]);
    }

    #[test]
    fn chart_paths_split_segments_on_missing_samples() {
        let t0 = Instant::now();
        let mut hist = BoundedHistory::new(10);
        hist.push(t0, Some(20.0));
        hist.push(t0 + Duration::from_secs(1), Some(40.0));
        hist.push(t0 + Duration::from_secs(2), None);
        hist.push(t0 + Duration::from_secs(3), Some(60.0));
        hist.push(t0 + Duration::from_secs(4), Some(80.0));

        let now = t0 + Duration::from_secs(4);
        let paths = hist.build_chart_paths(
            now,
            Duration::from_secs(60),
            600.0,
            100.0,
            0.0,
            100.0,
            Duration::from_secs(3),
        );

        assert!(paths.has_data);
        assert_eq!(paths.min_value, Some(20.0));
        assert_eq!(paths.max_value, Some(80.0));
        assert_eq!(paths.latest_value, Some(80.0));
        // Must contain two distinct 'M' subpaths due to the None gap.
        let m_count = paths.line_commands.matches("M ").count();
        assert_eq!(m_count, 2, "commands: {}", paths.line_commands);
    }

    #[test]
    fn chart_paths_split_segments_on_large_time_gap() {
        let t0 = Instant::now();
        let mut hist = BoundedHistory::new(10);
        hist.push(t0, Some(10.0));
        hist.push(t0 + Duration::from_secs(1), Some(20.0));
        hist.push(t0 + Duration::from_secs(20), Some(50.0));
        hist.push(t0 + Duration::from_secs(21), Some(60.0));

        let now = t0 + Duration::from_secs(21);
        let paths = hist.build_chart_paths(
            now,
            Duration::from_secs(60),
            600.0,
            100.0,
            0.0,
            100.0,
            Duration::from_secs(5),
        );

        assert_eq!(paths.line_commands.matches("M ").count(), 2);
    }

    #[test]
    fn chart_paths_empty_when_all_samples_unavailable() {
        let t0 = Instant::now();
        let mut hist = BoundedHistory::new(5);
        hist.push(t0, None);
        hist.push(t0 + Duration::from_secs(1), None);

        let paths = hist.build_chart_paths(
            t0 + Duration::from_secs(2),
            Duration::from_secs(60),
            600.0,
            100.0,
            0.0,
            100.0,
            Duration::from_secs(3),
        );

        assert!(!paths.has_data);
        assert!(paths.line_commands.is_empty());
        assert!(paths.area_commands.is_empty());
    }
}
