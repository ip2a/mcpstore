use serde::Serialize;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HealthSample {
    pub(crate) observed_at: f64,
    pub(crate) succeeded: bool,
    pub(crate) latency_ms: Option<f64>,
}

impl HealthSample {
    pub(crate) fn new(observed_at: f64, succeeded: bool, latency_ms: Option<f64>) -> Self {
        Self {
            observed_at,
            succeeded,
            latency_ms: latency_ms.filter(|value| value.is_finite() && *value >= 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub(crate) struct WindowStats {
    pub(crate) error_rate: Option<f64>,
    pub(crate) latency_p95: Option<f64>,
    pub(crate) latency_p99: Option<f64>,
    pub(crate) sample_size: usize,
}

#[derive(Debug)]
pub(crate) struct HealthWindow {
    duration_secs: f64,
    max_samples: usize,
    samples: VecDeque<HealthSample>,
}

impl HealthWindow {
    pub(crate) fn new(duration_secs: f64, max_samples: usize) -> Self {
        assert!(duration_secs.is_finite() && duration_secs > 0.0);
        assert!(max_samples > 0);
        Self {
            duration_secs,
            max_samples,
            samples: VecDeque::with_capacity(max_samples),
        }
    }

    pub(crate) fn record(&mut self, sample: HealthSample, now: f64) -> WindowStats {
        self.evict_expired(now);
        self.samples.push_back(sample);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
        self.stats(now)
    }

    pub(crate) fn stats(&mut self, now: f64) -> WindowStats {
        self.evict_expired(now);
        if self.samples.is_empty() {
            return WindowStats::default();
        }

        let sample_size = self.samples.len();
        let failures = self
            .samples
            .iter()
            .filter(|sample| !sample.succeeded)
            .count();
        let mut latencies: Vec<f64> = self
            .samples
            .iter()
            .filter_map(|sample| sample.latency_ms)
            .collect();
        latencies.sort_by(f64::total_cmp);

        WindowStats {
            error_rate: Some(failures as f64 / sample_size as f64),
            latency_p95: percentile(&latencies, 0.95),
            latency_p99: percentile(&latencies, 0.99),
            sample_size,
        }
    }

    fn evict_expired(&mut self, now: f64) {
        let cutoff = now - self.duration_secs;
        while self
            .samples
            .front()
            .is_some_and(|sample| sample.observed_at < cutoff)
        {
            self.samples.pop_front();
        }
    }
}

fn percentile(sorted: &[f64], quantile: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (quantile * sorted.len() as f64).ceil() as usize;
    Some(sorted[rank.saturating_sub(1).min(sorted.len() - 1)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(at: f64, succeeded: bool, latency_ms: f64) -> HealthSample {
        HealthSample::new(at, succeeded, Some(latency_ms))
    }

    #[test]
    fn empty_window_has_no_metrics() {
        let mut window = HealthWindow::new(10.0, 20);
        assert_eq!(window.stats(10.0), WindowStats::default());
    }

    #[test]
    fn records_error_rate_and_nearest_rank_percentiles() {
        let mut window = HealthWindow::new(60.0, 20);
        for index in 1..=20 {
            window.record(sample(index as f64, index > 5, index as f64), 20.0);
        }

        let stats = window.stats(20.0);
        assert_eq!(stats.sample_size, 20);
        assert_eq!(stats.error_rate, Some(0.25));
        assert_eq!(stats.latency_p95, Some(19.0));
        assert_eq!(stats.latency_p99, Some(20.0));
    }

    #[test]
    fn evicts_samples_outside_time_window_and_keeps_boundary() {
        let mut window = HealthWindow::new(10.0, 20);
        window.record(sample(0.0, false, 100.0), 0.0);
        window.record(sample(1.0, true, 10.0), 1.0);
        window.record(sample(11.0, true, 20.0), 11.0);

        let stats = window.stats(11.0);
        assert_eq!(stats.sample_size, 2);
        assert_eq!(stats.error_rate, Some(0.0));
        assert_eq!(stats.latency_p95, Some(20.0));
    }

    #[test]
    fn enforces_sample_capacity() {
        let mut window = HealthWindow::new(60.0, 2);
        window.record(sample(1.0, false, 1.0), 1.0);
        window.record(sample(2.0, true, 2.0), 2.0);
        let stats = window.record(sample(3.0, true, 3.0), 3.0);

        assert_eq!(stats.sample_size, 2);
        assert_eq!(stats.error_rate, Some(0.0));
        assert_eq!(stats.latency_p99, Some(3.0));
    }

    #[test]
    fn ignores_invalid_latency_without_dropping_outcome() {
        let mut window = HealthWindow::new(60.0, 20);
        window.record(HealthSample::new(1.0, false, Some(f64::NAN)), 1.0);
        window.record(HealthSample::new(2.0, true, Some(-1.0)), 2.0);

        let stats = window.stats(2.0);
        assert_eq!(stats.sample_size, 2);
        assert_eq!(stats.error_rate, Some(0.5));
        assert_eq!(stats.latency_p95, None);
        assert_eq!(stats.latency_p99, None);
    }

    #[test]
    fn single_sample_defines_both_percentiles() {
        let mut window = HealthWindow::new(60.0, 20);
        let stats = window.record(sample(1.0, true, 12.5), 1.0);
        assert_eq!(stats.latency_p95, Some(12.5));
        assert_eq!(stats.latency_p99, Some(12.5));
    }
}
