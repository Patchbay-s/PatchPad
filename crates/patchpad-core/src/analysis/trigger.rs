//! Plage d'une gâchette analogique : course minimale et maximale atteintes.

use serde::Serialize;

/// Seuils de « course complète » (en dessous / au-dessus).
const RELEASED_MAX: f32 = 0.02;
const PRESSED_MIN: f32 = 0.98;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerReport {
    pub min: f32,
    pub max: f32,
    /// La gâchette revient bien à zéro.
    pub reaches_zero: bool,
    /// La gâchette atteint bien le maximum.
    pub reaches_full: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TriggerAnalyzer {
    range: Option<(f32, f32)>,
}

impl TriggerAnalyzer {
    pub fn record(&mut self, value: f32) {
        self.range = Some(match self.range {
            Some((min, max)) => (min.min(value), max.max(value)),
            None => (value, value),
        });
    }

    pub fn report(&self) -> Option<TriggerReport> {
        self.range.map(|(min, max)| TriggerReport {
            min,
            max,
            reaches_zero: min <= RELEASED_MAX,
            reaches_full: max >= PRESSED_MIN,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_travel() {
        let mut t = TriggerAnalyzer::default();
        assert_eq!(t.report(), None);
        for v in [0.0, 0.3, 1.0, 0.5] {
            t.record(v);
        }
        let r = t.report().unwrap();
        assert_eq!((r.min, r.max), (0.0, 1.0));
        assert!(r.reaches_zero && r.reaches_full);
    }

    #[test]
    fn worn_trigger_never_reaches_max_nor_zero() {
        let mut t = TriggerAnalyzer::default();
        for v in [0.05, 0.4, 0.93] {
            t.record(v);
        }
        let r = t.report().unwrap();
        assert!(!r.reaches_zero && !r.reaches_full);
    }
}
