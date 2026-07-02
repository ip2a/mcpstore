use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

const MAX_LATENCY_SAMPLES: usize = 2048;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheRequestMetricsSnapshot {
    pub available: bool,
    pub scope: String,
    pub total_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub errors: u64,
    pub hit_rate: Option<f64>,
    pub avg_latency_ms: Option<f64>,
    pub p50_latency_ms: Option<f64>,
    pub p95_latency_ms: Option<f64>,
    pub p99_latency_ms: Option<f64>,
}

#[derive(Debug, Default)]
pub(crate) struct CacheRequestMetrics {
    total_requests: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    errors: AtomicU64,
    total_latency_micros: AtomicU64,
    latency_samples_micros: Mutex<Vec<u64>>,
}

impl CacheRequestMetrics {
    pub(crate) fn record(&self, latency: Duration, hit: Option<bool>, error: bool) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if error {
            self.errors.fetch_add(1, Ordering::Relaxed);
        }
        match hit {
            Some(true) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
            }
            Some(false) => {
                self.misses.fetch_add(1, Ordering::Relaxed);
            }
            None => {}
        }

        let micros = latency.as_micros().min(u128::from(u64::MAX)) as u64;
        self.total_latency_micros
            .fetch_add(micros, Ordering::Relaxed);
        let mut samples = self
            .latency_samples_micros
            .lock()
            .expect("cache metrics latency lock poisoned");
        if samples.len() >= MAX_LATENCY_SAMPLES {
            samples.remove(0);
        }
        samples.push(micros);
    }

    pub(crate) fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.errors.store(0, Ordering::Relaxed);
        self.total_latency_micros.store(0, Ordering::Relaxed);
        self.latency_samples_micros
            .lock()
            .expect("cache metrics latency lock poisoned")
            .clear();
    }

    pub(crate) fn snapshot(&self) -> CacheRequestMetricsSnapshot {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);
        let total_latency_micros = self.total_latency_micros.load(Ordering::Relaxed);
        let mut samples = self
            .latency_samples_micros
            .lock()
            .expect("cache metrics latency lock poisoned")
            .clone();
        samples.sort_unstable();

        let measured = hits + misses;
        CacheRequestMetricsSnapshot {
            available: true,
            scope: "process".to_string(),
            total_requests,
            hits,
            misses,
            errors,
            hit_rate: if measured == 0 {
                None
            } else {
                Some(hits as f64 / measured as f64)
            },
            avg_latency_ms: if total_requests == 0 {
                None
            } else {
                Some(total_latency_micros as f64 / total_requests as f64 / 1000.0)
            },
            p50_latency_ms: percentile_ms(&samples, 50),
            p95_latency_ms: percentile_ms(&samples, 95),
            p99_latency_ms: percentile_ms(&samples, 99),
        }
    }
}

fn percentile_ms(samples: &[u64], percentile: usize) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let rank = ((samples.len() * percentile).div_ceil(100)).saturating_sub(1);
    samples.get(rank).map(|micros| *micros as f64 / 1000.0)
}
