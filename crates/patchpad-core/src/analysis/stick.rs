//! Analyse d'un stick : valeur au repos, drift, zone morte conseillée, circularité.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::model::Stick;

/// Fenêtre sur laquelle on juge si le stick est immobile.
const REST_WINDOW: Duration = Duration::from_millis(500);
/// Dispersion max (rayon autour de la moyenne) pour considérer le stick immobile.
const REST_MAX_SPREAD: f32 = 0.03;
/// Au-delà, le stick est tenu volontairement, pas « au repos ».
const REST_MAX_OFFSET: f32 = 0.35;
const MIN_REST_SAMPLES: usize = 5;
/// Marge ajoutée au rayon observé au repos pour conseiller une zone morte.
const DEADZONE_MARGIN: f32 = 0.02;

/// Secteurs angulaires pour la circularité.
pub const CIRCULARITY_SECTORS: usize = 48;
/// Seuls les points proches du bord comptent pour la circularité.
const CIRCULARITY_MIN_RADIUS: f32 = 0.5;
/// Couverture minimale avant de donner un score.
const CIRCULARITY_MIN_COVERAGE: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DriftVerdict {
    /// Pas encore de période de repos observée.
    Unknown,
    Ok,
    Drift,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickReport {
    /// Position moyenne lors du dernier repos.
    pub rest: Option<Stick>,
    /// Distance entre la position de repos et le centre.
    pub rest_offset: Option<f32>,
    /// Plus grand écart au centre observé au repos (décalage + bruit).
    pub rest_radius: Option<f32>,
    pub suggested_deadzone: Option<f32>,
    pub drift: DriftVerdict,
    pub at_rest: bool,
    /// Rayon max par secteur angulaire (0 = secteur non visité), secteur 0 = +X, sens trigo.
    pub sectors: Vec<f32>,
    /// Part des secteurs visités, dans [0, 1].
    pub coverage: f32,
    /// Écart moyen au cercle unité (en %), si la couverture est suffisante.
    pub circularity_error: Option<f32>,
    /// Rayon max atteint (1 = bord du cercle).
    pub max_radius: f32,
}

#[derive(Debug, Clone)]
pub struct StickAnalyzer {
    window: VecDeque<(Instant, Stick)>,
    rest: Option<Stick>,
    rest_radius: f32,
    at_rest: bool,
    sectors: [f32; CIRCULARITY_SECTORS],
    max_radius: f32,
}

impl Default for StickAnalyzer {
    fn default() -> Self {
        StickAnalyzer {
            window: VecDeque::new(),
            rest: None,
            rest_radius: 0.0,
            at_rest: false,
            sectors: [0.0; CIRCULARITY_SECTORS],
            max_radius: 0.0,
        }
    }
}

impl StickAnalyzer {
    pub fn record(&mut self, at: Instant, stick: Stick) {
        self.window.push_back((at, stick));
        while self.window.front().is_some_and(|(t, _)| at.saturating_duration_since(*t) > REST_WINDOW) {
            self.window.pop_front();
        }
        self.update_rest(at);

        let radius = stick.radius();
        self.max_radius = self.max_radius.max(radius);
        if radius >= CIRCULARITY_MIN_RADIUS {
            let sector = sector_of(stick);
            if let Some(slot) = self.sectors.get_mut(sector) {
                *slot = slot.max(radius);
            }
        }
    }

    fn update_rest(&mut self, at: Instant) {
        // Fenêtre trop courte (début, ou rapports rares) : on garde le dernier verdict.
        let covered = self.window.front().is_some_and(|(t, _)| at.saturating_duration_since(*t) >= REST_WINDOW * 4 / 5);
        if self.window.len() < MIN_REST_SAMPLES || !covered {
            return;
        }
        #[allow(clippy::cast_precision_loss)]
        let n = self.window.len() as f32;
        let mean = Stick {
            x: self.window.iter().map(|(_, s)| s.x).sum::<f32>() / n,
            y: self.window.iter().map(|(_, s)| s.y).sum::<f32>() / n,
        };
        let spread = self.window.iter().map(|(_, s)| (s.x - mean.x).hypot(s.y - mean.y)).fold(0.0f32, f32::max);
        self.at_rest = spread <= REST_MAX_SPREAD && mean.radius() <= REST_MAX_OFFSET;
        if self.at_rest {
            let radius = self.window.iter().map(|(_, s)| s.radius()).fold(0.0f32, f32::max);
            self.rest_radius = if self.rest.is_some() { self.rest_radius.max(radius) } else { radius };
            self.rest = Some(mean);
        }
    }

    pub fn report(&self, drift_threshold: f32) -> StickReport {
        let rest_offset = self.rest.map(Stick::radius);
        let drift = match rest_offset {
            None => DriftVerdict::Unknown,
            Some(offset) if offset > drift_threshold => DriftVerdict::Drift,
            Some(_) => DriftVerdict::Ok,
        };
        let visited: Vec<f32> = self.sectors.iter().copied().filter(|r| *r > 0.0).collect();
        #[allow(clippy::cast_precision_loss)]
        let coverage = visited.len() as f32 / CIRCULARITY_SECTORS as f32;
        #[allow(clippy::cast_precision_loss)]
        let circularity_error = (coverage >= CIRCULARITY_MIN_COVERAGE)
            .then(|| visited.iter().map(|r| (r - 1.0).abs()).sum::<f32>() / visited.len() as f32 * 100.0);
        StickReport {
            rest: self.rest,
            rest_offset,
            rest_radius: self.rest.map(|_| self.rest_radius),
            suggested_deadzone: self.rest.map(|_| suggested_deadzone(self.rest_radius)),
            drift,
            at_rest: self.at_rest,
            sectors: self.sectors.to_vec(),
            coverage,
            circularity_error,
            max_radius: self.max_radius,
        }
    }
}

/// Zone morte conseillée : rayon observé au repos + marge, arrondi au centième supérieur.
pub fn suggested_deadzone(rest_radius: f32) -> f32 {
    // Le epsilon évite qu'une erreur d'arrondi flottant (0,12 → 12,000001) ajoute un centième.
    ((rest_radius + DEADZONE_MARGIN) * 100.0 - 1e-3).ceil() / 100.0
}

fn sector_of(stick: Stick) -> usize {
    let angle = stick.y.atan2(stick.x).rem_euclid(std::f32::consts::TAU);
    #[allow(clippy::cast_precision_loss)]
    let scaled = (angle / std::f32::consts::TAU * CIRCULARITY_SECTORS as f32).floor();
    // `scaled` est dans [0, SECTORS] : la conversion est exacte.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sector = scaled as usize;
    sector % CIRCULARITY_SECTORS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(analyzer: &mut StickAnalyzer, start: Instant, from_ms: u64, to_ms: u64, f: impl Fn(u64) -> Stick) {
        for ms in (from_ms..to_ms).step_by(4) {
            analyzer.record(start + Duration::from_millis(ms), f(ms));
        }
    }

    #[test]
    fn centered_stick_has_no_drift() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        feed(&mut a, start, 0, 1000, |ms| Stick { x: if ms % 8 == 0 { 0.004 } else { -0.004 }, y: 0.0 });
        let r = a.report(0.05);
        assert!(r.at_rest);
        assert_eq!(r.drift, DriftVerdict::Ok);
        assert!(r.rest_offset.unwrap() < 0.001);
        assert!((r.rest_radius.unwrap() - 0.004).abs() < 1e-6);
        assert_eq!(r.suggested_deadzone, Some(0.03));
    }

    #[test]
    fn offset_rest_position_is_drift() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        feed(&mut a, start, 0, 1000, |_| Stick { x: 0.06, y: -0.08 });
        let r = a.report(0.05);
        assert_eq!(r.drift, DriftVerdict::Drift);
        assert!((r.rest_offset.unwrap() - 0.1).abs() < 1e-5);
        assert_eq!(r.suggested_deadzone, Some(0.12));
        // Le même décalage passe sous un seuil plus tolérant.
        assert_eq!(a.report(0.15).drift, DriftVerdict::Ok);
    }

    #[test]
    fn moving_or_held_stick_is_not_rest() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        feed(&mut a, start, 0, 1000, |ms| {
            let t = ms as f32 / 100.0;
            Stick { x: t.cos(), y: t.sin() }
        });
        let r = a.report(0.05);
        assert!(!r.at_rest);
        assert_eq!(r.drift, DriftVerdict::Unknown);

        let mut held = StickAnalyzer::default();
        feed(&mut held, start, 0, 1000, |_| Stick { x: 0.9, y: 0.0 });
        assert_eq!(held.report(0.05).drift, DriftVerdict::Unknown);
    }

    #[test]
    fn perfect_circle_scores_zero_error() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        for i in 0..360u32 {
            let angle = (i as f32).to_radians();
            a.record(start + Duration::from_millis(u64::from(i)), Stick { x: angle.cos(), y: angle.sin() });
        }
        let r = a.report(0.05);
        assert_eq!(r.coverage, 1.0);
        assert!(r.circularity_error.unwrap() < 0.01);
        assert!((r.max_radius - 1.0).abs() < 1e-5);
    }

    #[test]
    fn square_gate_has_large_error() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        for i in 0..360u32 {
            let angle = (i as f32).to_radians();
            let (x, y) = (angle.cos(), angle.sin());
            let k = 1.0 / x.abs().max(y.abs());
            a.record(start + Duration::from_millis(u64::from(i)), Stick { x: x * k, y: y * k });
        }
        let err = a.report(0.05).circularity_error.unwrap();
        assert!(err > 10.0 && err < 20.0, "{err}");
    }

    #[test]
    fn partial_coverage_gives_no_score() {
        let start = Instant::now();
        let mut a = StickAnalyzer::default();
        for i in 0..90u32 {
            let angle = (i as f32).to_radians();
            a.record(start + Duration::from_millis(u64::from(i)), Stick { x: angle.cos(), y: angle.sin() });
        }
        let r = a.report(0.05);
        assert!((r.coverage - 0.25).abs() < 0.03, "{}", r.coverage);
        assert_eq!(r.circularity_error, None);
    }

    #[test]
    fn sectors_follow_trigonometric_order() {
        assert_eq!(sector_of(Stick { x: 1.0, y: 0.0 }), 0);
        assert_eq!(sector_of(Stick { x: 0.0, y: 1.0 }), CIRCULARITY_SECTORS / 4);
        assert_eq!(sector_of(Stick { x: -1.0, y: -0.0001 }), CIRCULARITY_SECTORS / 2);
        assert_eq!(sector_of(Stick { x: 0.0, y: -1.0 }), CIRCULARITY_SECTORS * 3 / 4);
    }
}
