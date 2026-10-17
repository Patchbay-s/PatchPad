//! Interface commune aux sources de manettes (hidapi pour la DS4, gilrs pour le reste).

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::model::{ControllerInfo, ControllerState};

/// Événement normalisé produit par un backend.
#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    Connected(ControllerInfo),
    /// Les métadonnées ont changé (mode de rapport, adaptateur appairé…).
    InfoChanged(ControllerInfo),
    Disconnected(String),
    /// Un rapport d'entrée a été reçu. `at` sert au calcul de la fréquence.
    Input {
        id: String,
        state: ControllerState,
        at: Instant,
    },
}

/// Problème d'environnement à afficher à l'utilisateur (le frontend traduit `code`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub code: NoticeCode,
    /// Détail technique (chemin, message d'erreur de l'OS…).
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NoticeCode {
    /// Linux : accès hidraw refusé, il manque la règle udev.
    HidrawPermission,
    /// macOS : l'ouverture du périphérique HID a été refusée.
    MacHidDenied,
    /// Échec d'ouverture HID pour une autre raison.
    HidOpenFailed,
    /// hidapi n'a pas pu s'initialiser.
    HidUnavailable,
    /// gilrs n'a pas pu s'initialiser.
    GilrsUnavailable,
    /// macOS : manette Xbox branchée en USB (protocole GIP, non HID).
    XboxUsbMacUnsupported,
    /// DS4 Bluetooth restée en mode réduit malgré la demande de rapport complet.
    Ds4ReducedMode,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OutputError {
    #[error("manette inconnue : {0}")]
    UnknownController(String),
    #[error("sortie non prise en charge sur cet OS ou cette manette")]
    Unsupported,
    #[error("écriture vers la manette impossible : {0}")]
    Write(String),
}

/// Commande de vibration. Les intensités sont dans [0, 1].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rumble {
    /// Moteur basse fréquence (gauche, « fort »).
    pub strong: f32,
    /// Moteur haute fréquence (droite, « faible »).
    pub weak: f32,
    pub duration: Duration,
}

/// Source de manettes. Les implémentations ne sont pas forcément `Send` (gilrs ne
/// l'est pas sur toutes les plateformes) : le gestionnaire les crée dans son thread.
pub trait ControllerBackend {
    fn name(&self) -> &'static str;

    /// Lit sans bloquer tout ce qui est disponible et ajoute les événements à `out`.
    fn poll(&mut self, now: Instant, out: &mut Vec<BackendEvent>);

    /// Problèmes d'environnement actuellement actifs.
    fn notices(&self) -> Vec<Notice> {
        Vec::new()
    }

    /// Couples VID/PID ouverts par ce backend, que les backends de priorité
    /// inférieure doivent ignorer pour éviter les doublons.
    fn claimed_devices(&self) -> Vec<(u16, u16)> {
        Vec::new()
    }

    /// Couples VID/PID déjà pris en charge par un backend prioritaire.
    fn set_excluded_devices(&mut self, _devices: &[(u16, u16)]) {}

    fn owns(&self, id: &str) -> bool;

    fn rumble(&mut self, id: &str, rumble: Rumble, now: Instant) -> Result<(), OutputError>;

    fn set_lightbar(&mut self, _id: &str, _rgb: [u8; 3]) -> Result<(), OutputError> {
        Err(OutputError::Unsupported)
    }
}

/// Convertit une intensité [0, 1] en octet.
pub(crate) fn unit_to_u8(value: f32) -> u8 {
    let scaled = (value.clamp(0.0, 1.0) * 255.0).round();
    // Borné à [0, 255] juste au-dessus : la conversion est exacte.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = scaled as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_to_u8_clamps_and_rounds() {
        assert_eq!(unit_to_u8(-1.0), 0);
        assert_eq!(unit_to_u8(0.5), 128);
        assert_eq!(unit_to_u8(1.0), 255);
        assert_eq!(unit_to_u8(7.0), 255);
        assert_eq!(unit_to_u8(f32::NAN), 0);
    }
}
