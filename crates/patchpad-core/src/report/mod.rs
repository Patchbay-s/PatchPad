//! Mode rapport : suivi de ce qui a été testé pendant une session, puis export HTML ou PDF.
//!
//! - [`ReportProgress`] est alimenté par le gestionnaire à chaque rapport d'entrée
//!   (pleine fréquence : un appui bref n'échappe pas au suivi).
//! - [`document`] construit un document neutre (sections, lignes, verdicts) à partir de l'état
//!   et des analyses ; [`html`] et [`pdf`] le mettent en forme.

pub mod document;
pub mod html;
pub mod pdf;
pub mod text;

use serde::{Deserialize, Serialize};

use crate::manager::ControllerView;
use crate::model::{Button, ControllerState, Family};

/// Au-delà, on considère que la manette a été bougée (°/s).
pub const GYRO_MOVED_DPS: f32 = 45.0;
/// En dessous, la manette est considérée immobile pour mesurer la gravité (°/s).
const GYRO_STILL_DPS: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Untested,
    /// Impossible à tester sur cet OS ou avec cette manette.
    Unavailable,
    /// Valeur informative, sans verdict.
    Info,
}

/// Retour de l'utilisateur pour une sortie (il est seul à savoir si ça a vibré).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confirmation {
    #[default]
    Untested,
    Ok,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputItem {
    RumbleLeft,
    RumbleRight,
    Lightbar,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ButtonCheck {
    pub button: Button,
    /// `false` : bouton absent ou illisible sur cet OS (bouton Xbox via XInput…).
    pub available: bool,
    pub presses: u32,
    /// Millisecondes depuis le début du rapport.
    pub first_press_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TouchCheck {
    pub finger_seen: [bool; 2],
    pub both_at_once: bool,
    pub clicked: bool,
    /// Étendue couverte (min, max) en X et en Y.
    pub x_range: Option<(u16, u16)>,
    pub y_range: Option<(u16, u16)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImuCheck {
    pub samples: u64,
    /// Valeur absolue max par axe (°/s).
    pub max_gyro: [f32; 3],
    /// Norme moyenne de l'accéléromètre quand la manette est immobile (g).
    pub accel_rest_norm: Option<f32>,
    #[serde(skip)]
    rest_sum: f64,
    #[serde(skip)]
    rest_count: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputChecks {
    pub rumble_left: Confirmation,
    pub rumble_right: Confirmation,
    pub lightbar: Confirmation,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportProgress {
    pub buttons: Vec<ButtonCheck>,
    pub touchpad: Option<TouchCheck>,
    pub imu: Option<ImuCheck>,
    pub outputs: OutputChecks,
    /// Boutons testés / boutons testables.
    pub buttons_tested: usize,
    pub buttons_total: usize,
    #[serde(skip)]
    previous: Option<crate::model::Buttons>,
}

/// Boutons attendus selon la famille de manette.
pub fn expected_buttons(family: Family) -> Vec<(Button, bool)> {
    Button::ALL
        .into_iter()
        .filter(|b| family == Family::DualShock4 || *b != Button::Touchpad)
        .map(|b| (b, button_available(family, b)))
        .collect()
}

/// XInput (Windows) n'expose pas le bouton Guide des manettes Xbox.
fn button_available(family: Family, button: Button) -> bool {
    !(cfg!(target_os = "windows") && family != Family::DualShock4 && button == Button::Home)
}

impl ReportProgress {
    pub fn new(family: Family, has_touchpad: bool, has_imu: bool) -> Self {
        let buttons: Vec<ButtonCheck> = expected_buttons(family)
            .into_iter()
            .map(|(button, available)| ButtonCheck { button, available, presses: 0, first_press_ms: None })
            .collect();
        let buttons_total = buttons.iter().filter(|b| b.available).count();
        ReportProgress {
            buttons,
            touchpad: has_touchpad.then(TouchCheck::default),
            imu: has_imu.then(ImuCheck::default),
            outputs: OutputChecks::default(),
            buttons_tested: 0,
            buttons_total,
            previous: None,
        }
    }

    /// `t_ms` : millisecondes depuis le début du rapport.
    pub fn record(&mut self, state: &ControllerState, t_ms: u64) {
        let previous = self.previous.unwrap_or_default();
        for check in &mut self.buttons {
            if state.buttons.contains(check.button) && !previous.contains(check.button) {
                check.presses += 1;
                check.first_press_ms.get_or_insert(t_ms);
            }
        }
        self.previous = Some(state.buttons);
        self.buttons_tested = self.buttons.iter().filter(|b| b.available && b.presses > 0).count();

        if let (Some(check), Some(touchpad)) = (&mut self.touchpad, &state.touchpad) {
            let active: Vec<_> = touchpad.fingers.iter().filter(|f| f.active).collect();
            for (seen, finger) in check.finger_seen.iter_mut().zip(&touchpad.fingers) {
                *seen |= finger.active;
            }
            check.both_at_once |= active.len() == 2;
            check.clicked |= touchpad.clicked;
            for finger in active {
                check.x_range = Some(widen(check.x_range, finger.x));
                check.y_range = Some(widen(check.y_range, finger.y));
            }
        }

        if let (Some(check), Some(imu)) = (&mut self.imu, &state.imu) {
            check.samples += 1;
            for (max, value) in check.max_gyro.iter_mut().zip(imu.gyro) {
                *max = max.max(value.abs());
            }
            if imu.gyro.iter().all(|g| g.abs() < GYRO_STILL_DPS) {
                let [x, y, z] = imu.accel;
                check.rest_sum += f64::from((x * x + y * y + z * z).sqrt());
                check.rest_count += 1;
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
                let mean = (check.rest_sum / check.rest_count as f64) as f32;
                check.accel_rest_norm = Some(mean);
            }
        }
    }

    pub fn confirm(&mut self, item: OutputItem, result: Confirmation) {
        match item {
            OutputItem::RumbleLeft => self.outputs.rumble_left = result,
            OutputItem::RumbleRight => self.outputs.rumble_right = result,
            OutputItem::Lightbar => self.outputs.lightbar = result,
        }
    }
}

fn widen(range: Option<(u16, u16)>, value: u16) -> (u16, u16) {
    match range {
        Some((min, max)) => (min.min(value), max.max(value)),
        None => (value, value),
    }
}

/// Tout ce qu'il faut pour produire un rapport, figé au moment de l'export.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportData {
    pub view: ControllerView,
    pub drift_threshold: f32,
    /// Durée de la session de test, en secondes.
    pub duration_s: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Finger, Imu, Touchpad};

    #[test]
    fn counts_presses_once_per_press() {
        let mut progress = ReportProgress::new(Family::DualShock4, true, true);
        assert_eq!(progress.buttons.len(), 18);
        let pressed = ControllerState { buttons: [Button::South].into_iter().collect(), ..Default::default() };
        let released = ControllerState::default();
        progress.record(&pressed, 10);
        progress.record(&pressed, 14);
        progress.record(&released, 18);
        progress.record(&pressed, 30);
        let south = progress.buttons.iter().find(|b| b.button == Button::South).unwrap();
        assert_eq!((south.presses, south.first_press_ms), (2, Some(10)));
        assert_eq!(progress.buttons_tested, 1);
    }

    #[test]
    fn xbox_has_no_touchpad_button() {
        let progress = ReportProgress::new(Family::Xbox, false, false);
        assert!(progress.buttons.iter().all(|b| b.button != Button::Touchpad));
        assert!(progress.touchpad.is_none() && progress.imu.is_none());
    }

    #[test]
    fn touch_and_imu_checks() {
        let mut progress = ReportProgress::new(Family::DualShock4, true, true);
        let finger = |x, y| Finger { active: true, id: 1, x, y };
        let state = ControllerState {
            touchpad: Some(Touchpad {
                fingers: [finger(100, 50), finger(1800, 900)],
                clicked: true,
                width: 1920,
                height: 942,
            }),
            imu: Some(Imu { gyro: [0.5, -120.0, 1.0], accel: [0.0, 1.0, 0.0] }),
            ..Default::default()
        };
        progress.record(&state, 0);
        let still = ControllerState { imu: Some(Imu { gyro: [0.1, 0.2, 0.0], accel: [0.0, 0.98, 0.0] }), ..state };
        progress.record(&still, 4);

        let touch = progress.touchpad.unwrap();
        assert_eq!(touch.finger_seen, [true, true]);
        assert!(touch.both_at_once && touch.clicked);
        assert_eq!(touch.x_range, Some((100, 1800)));
        assert_eq!(touch.y_range, Some((50, 900)));

        let imu = progress.imu.unwrap();
        assert_eq!(imu.samples, 2);
        assert_eq!(imu.max_gyro, [0.5, 120.0, 1.0]);
        // Seul l'échantillon immobile compte pour la gravité.
        assert!((imu.accel_rest_norm.unwrap() - 0.98).abs() < 1e-4);
    }
}
