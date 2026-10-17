//! État normalisé d'une manette, commun à tous les backends.
//!
//! Conventions : sticks dans [-1, 1] avec +Y vers le haut, gâchettes dans [0, 1],
//! gyroscope en °/s, accéléromètre en g.

use serde::Serialize;

/// Boutons normalisés. Les libellés affichés dépendent de la famille de manette
/// (✕/○/□/△ pour la DS4, A/B/X/Y pour la Xbox).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Button {
    /// ✕ / A
    South,
    /// ○ / B
    East,
    /// □ / X
    West,
    /// △ / Y
    North,
    L1,
    R1,
    /// Bouton numérique de la gâchette gauche (DS4 uniquement).
    L2,
    /// Bouton numérique de la gâchette droite (DS4 uniquement).
    R2,
    /// Share / View
    Select,
    /// Options / Menu
    Start,
    L3,
    R3,
    /// PS / Xbox
    Home,
    /// Clic du pavé tactile (DS4).
    Touchpad,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
}

impl Button {
    pub const ALL: [Button; 18] = [
        Button::South,
        Button::East,
        Button::West,
        Button::North,
        Button::L1,
        Button::R1,
        Button::L2,
        Button::R2,
        Button::Select,
        Button::Start,
        Button::L3,
        Button::R3,
        Button::Home,
        Button::Touchpad,
        Button::DpadUp,
        Button::DpadDown,
        Button::DpadLeft,
        Button::DpadRight,
    ];

    fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// Ensemble de boutons appuyés (bitmask), sérialisé comme une liste de noms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Buttons(u32);

impl Buttons {
    pub fn set(&mut self, button: Button, pressed: bool) {
        if pressed {
            self.0 |= button.bit();
        } else {
            self.0 &= !button.bit();
        }
    }

    pub fn contains(self, button: Button) -> bool {
        self.0 & button.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = Button> {
        Button::ALL.into_iter().filter(move |b| self.contains(*b))
    }
}

impl FromIterator<Button> for Buttons {
    fn from_iter<T: IntoIterator<Item = Button>>(iter: T) -> Self {
        let mut buttons = Buttons::default();
        for button in iter {
            buttons.set(button, true);
        }
        buttons
    }
}

impl Serialize for Buttons {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Stick {
    pub x: f32,
    pub y: f32,
}

impl Stick {
    pub fn radius(self) -> f32 {
        self.x.hypot(self.y)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finger {
    pub active: bool,
    /// Identifiant de contact (7 bits), incrémenté à chaque nouveau contact.
    pub id: u8,
    /// 0..=1919 sur une DS4.
    pub x: u16,
    /// 0..=941 sur une DS4.
    pub y: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Touchpad {
    pub fingers: [Finger; 2],
    pub clicked: bool,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imu {
    /// °/s, axes (x = tangage, y = lacet, z = roulis).
    pub gyro: [f32; 3],
    /// g, axes (x = droite, y = haut, z = vers le joueur).
    pub accel: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BatteryStatus {
    Discharging,
    Charging,
    Full,
    /// Alimentée par câble, sans information de charge.
    Wired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Battery {
    /// Pourcentage, si connu.
    pub level: Option<u8>,
    pub status: BatteryStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerState {
    pub buttons: Buttons,
    pub left_stick: Stick,
    pub right_stick: Stick,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub touchpad: Option<Touchpad>,
    pub imu: Option<Imu>,
    pub battery: Option<Battery>,
    /// Câble USB branché (DS4), si l'information existe.
    pub cable_connected: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Family {
    DualShock4,
    Xbox,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Connection {
    Usb,
    Bluetooth,
    /// Sans fil, sans distinction possible entre Bluetooth et adaptateur (XInput).
    Wireless,
    /// Adaptateur sans fil Sony (USB côté ordinateur, sans fil côté manette).
    Dongle,
    Unknown,
}

/// Disponibilité d'une sortie (vibration, barre lumineuse).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Support {
    Supported,
    /// Pas prise en charge par le pilote de cet OS.
    UnsupportedOs,
    /// La manette n'a pas ce matériel.
    NoHardware,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub touchpad: bool,
    pub imu: bool,
    pub rumble: Support,
    pub lightbar: Support,
    /// Les gâchettes sont analogiques.
    pub analog_triggers: bool,
}

/// Mode de rapport d'une DS4 en Bluetooth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportMode {
    /// Rapport complet (USB 0x01 ou Bluetooth 0x11).
    Full,
    /// Rapport Bluetooth réduit 0x01 : ni IMU, ni pavé tactile, ni batterie.
    Reduced,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerInfo {
    pub id: String,
    pub name: String,
    pub family: Family,
    /// « hidapi » ou « gilrs ».
    pub backend: &'static str,
    pub connection: Connection,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub capabilities: Capabilities,
    pub report_mode: Option<ReportMode>,
    /// Calibration IMU usine lue et valide (DS4).
    pub imu_factory_calibration: Option<bool>,
    /// Adaptateur DS4 sans manette appairée.
    pub dongle_idle: bool,
}
