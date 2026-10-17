//! Fréquence de rapport (Hz) sur une fenêtre glissante.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateStats {
    pub hz: f32,
    /// Intervalle moyen entre deux rapports, en ms.
    pub mean_interval_ms: f32,
    /// Écart type des intervalles, en ms.
    pub jitter_ms: f32,
    /// Plus grand intervalle observé dans la fenêtre, en ms.
    pub max_interval_ms: f32,
}

#[derive(Debug, Clone)]
pub struct RateMeter {
    window: Duration,
    stamps: VecDeque<Instant>,
}

impl Default for RateMeter {
    fn default() -> Self {
        Self::new(Duration::from_secs(1))
    }
}

impl RateMeter {
    pub fn new(window: Duration) -> Self {
        RateMeter { window, stamps: VecDeque::new() }
    }

    pub fn record(&mut self, at: Instant) {
        self.stamps.push_back(at);
        self.prune(at);
    }

    fn prune(&mut self, now: Instant) {
        while self.stamps.front().is_some_and(|t| now.saturating_duration_since(*t) > self.window) {
            self.stamps.pop_front();
        }
    }

    pub fn stats(&mut self, now: Instant) -> RateStats {
        self.prune(now);
        let (Some(first), Some(last)) = (self.stamps.front(), self.stamps.back()) else {
            return RateStats::default();
        };
        let span = last.saturating_duration_since(*first).as_secs_f32();
        let intervals = self.stamps.len().saturating_sub(1);
        if intervals == 0 || span <= 0.0 {
            return RateStats::default();
        }
        #[allow(clippy::cast_precision_loss)]
        let count = intervals as f32;
        let mean = span / count;
        let (sum_sq, max) =
            self.stamps.iter().zip(self.stamps.iter().skip(1)).fold((0.0f32, 0.0f32), |(sq, max), (a, b)| {
                let dt = b.saturating_duration_since(*a).as_secs_f32();
                (sq + (dt - mean).powi(2), max.max(dt))
            });
        RateStats {
            hz: 1.0 / mean,
            mean_interval_ms: mean * 1000.0,
            jitter_ms: (sum_sq / count).sqrt() * 1000.0,
            max_interval_ms: max * 1000.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_250_hz() {
        let start = Instant::now();
        let mut meter = RateMeter::default();
        for i in 0..500u32 {
            meter.record(start + Duration::from_millis(4) * i);
        }
        let stats = meter.stats(start + Duration::from_millis(4) * 499);
        assert!((stats.hz - 250.0).abs() < 0.5, "{stats:?}");
        assert!((stats.mean_interval_ms - 4.0).abs() < 0.01);
        assert!(stats.jitter_ms < 0.01);
        assert!((stats.max_interval_ms - 4.0).abs() < 0.01);
    }

    #[test]
    fn jitter_and_gap_are_reported() {
        let start = Instant::now();
        let mut meter = RateMeter::default();
        let mut t = start;
        for i in 0..100 {
            t += Duration::from_millis(if i % 2 == 0 { 2 } else { 6 });
            meter.record(t);
        }
        let stats = meter.stats(t);
        assert!((stats.hz - 250.0).abs() < 3.0, "{stats:?}");
        assert!((stats.jitter_ms - 2.0).abs() < 0.05, "{stats:?}");
        assert!((stats.max_interval_ms - 6.0).abs() < 0.01);
    }

    #[test]
    fn drops_to_zero_when_reports_stop() {
        let start = Instant::now();
        let mut meter = RateMeter::default();
        for i in 0..10u32 {
            meter.record(start + Duration::from_millis(10) * i);
        }
        assert!(meter.stats(start + Duration::from_millis(100)).hz > 0.0);
        assert_eq!(meter.stats(start + Duration::from_secs(5)), RateStats::default());
    }

    #[test]
    fn single_report_has_no_rate() {
        let start = Instant::now();
        let mut meter = RateMeter::default();
        meter.record(start);
        assert_eq!(meter.stats(start).hz, 0.0);
    }
}
