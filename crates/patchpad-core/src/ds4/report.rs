//! Parsing des rapports d'entrée bruts de la DualShock 4.
//!
//! Disposition vérifiée contre le driver Linux `drivers/hid/hid-playstation.c`
//! (`struct dualshock4_input_report_common`, `_usb`, `_bt`, `dualshock4_parse_report`)
//! et `src/joystick/hidapi/SDL_hidapi_ps4.c` (SDL 3). Rappel des structures :
//!
//! ```text
//! common (32 octets) : x, y, rx, ry, buttons[3], z, rz,
//!                      sensor_timestamp (le16), sensor_temperature,
//!                      gyro[3] (le16), accel[3] (le16), reserved2[5],
//!                      status[2], reserved3
//! usb  (64) : report_id 0x01, common, num_touch_reports, touch_reports[3], reserved[3]
//! bt   (78) : report_id 0x11, reserved[2], common, num_touch_reports,
//!             touch_reports[4], reserved2[2], crc32 (le32)
//! bt réduit (10) : report_id 0x01 + les 9 premiers octets de common
//! touch_report (9) : timestamp, points[2]
//! touch_point (4)  : contact (bit 7 = inactif, bits 0..6 = id), x_lo,
//!                    x_hi:4 | y_lo:4, y_hi
//! ```

use crate::model::{Battery, BatteryStatus, Button, Buttons, Finger, Stick, Touchpad};

pub const REPORT_ID_USB: u8 = 0x01;
pub const REPORT_ID_BT: u8 = 0x11;
/// `DS4_INPUT_REPORT_USB_SIZE`
pub const USB_REPORT_SIZE: usize = 64;
/// `DS4_INPUT_REPORT_BT_SIZE`
pub const BT_REPORT_SIZE: usize = 78;
/// `DS4_INPUT_REPORT_BT_MINIMAL_SIZE`
pub const BT_REDUCED_REPORT_SIZE: usize = 10;

/// `DS4_TOUCHPAD_WIDTH` / `DS4_TOUCHPAD_HEIGHT`
pub const TOUCHPAD_WIDTH: u16 = 1920;
pub const TOUCHPAD_HEIGHT: u16 = 942;

/// Graine CRC des rapports d'entrée Bluetooth (`PS_INPUT_CRC32_SEED`).
pub const INPUT_CRC_SEED: u8 = 0xA1;

/// Décalage du bloc `common` après l'identifiant de rapport.
const COMMON_OFFSET_USB: usize = 1;
/// En Bluetooth, 2 octets réservés précèdent `common`.
const COMMON_OFFSET_BT: usize = 3;

// Décalages dans `common`.
const C_X: usize = 0;
const C_Y: usize = 1;
const C_RX: usize = 2;
const C_RY: usize = 3;
const C_BUTTONS: usize = 4;
const C_Z: usize = 7;
const C_RZ: usize = 8;
const C_TIMESTAMP: usize = 9;
const C_GYRO: usize = 12;
const C_ACCEL: usize = 18;
const C_STATUS: usize = 29;
/// Après `common` (32 octets) : num_touch_reports puis les touch_reports.
const C_NUM_TOUCH: usize = 32;
const C_TOUCH: usize = 33;
const TOUCH_REPORT_SIZE: usize = 9;
const USB_TOUCH_REPORTS: usize = 3;
const BT_TOUCH_REPORTS: usize = 4;

// Masques (`DS_BUTTONS*`, `DS4_STATUS*`).
const HAT_MASK: u8 = 0x0F;
const STATUS0_BATTERY: u8 = 0x0F;
const STATUS0_CABLE: u8 = 1 << 4;
const STATUS1_DONGLE_DISCONNECTED: u8 = 1 << 2;
const BATTERY_FULL: u8 = 11;
const TOUCH_INACTIVE: u8 = 1 << 7;

/// Transport, déterminé à l'énumération (le rapport 0x01 a deux formats différents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Usb,
    Bluetooth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("rapport vide")]
    Empty,
    #[error("rapport 0x{id:02x} trop court : {len} octets")]
    TooShort { id: u8, len: usize },
    #[error("rapport 0x{0:02x} inattendu")]
    UnknownReport(u8),
    #[error("CRC32 du rapport Bluetooth invalide")]
    BadCrc,
}

/// Contenu d'un rapport d'entrée, valeurs brutes (non calibrées).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawInput {
    pub buttons: Buttons,
    /// Valeurs brutes 0..=255 : x, y, rx, ry (0 = gauche / haut).
    pub sticks: [u8; 4],
    /// L2, R2 : 0..=255.
    pub triggers: [u8; 2],
    /// Absent en mode Bluetooth réduit.
    pub full: Option<FullInput>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FullInput {
    /// Unités de 5,33 µs.
    pub sensor_timestamp: u16,
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
    pub touch_fingers: [Finger; 2],
    pub battery: Battery,
    pub cable_connected: bool,
    /// Adaptateur sans fil : bit « manette déconnectée » (`DS4_STATUS1_DONGLE_STATE`).
    pub dongle_disconnected: bool,
}

impl RawInput {
    pub fn left_stick(&self) -> Stick {
        Stick { x: axis(self.sticks[0]), y: -axis(self.sticks[1]) }
    }

    pub fn right_stick(&self) -> Stick {
        Stick { x: axis(self.sticks[2]), y: -axis(self.sticks[3]) }
    }

    pub fn triggers(&self) -> [f32; 2] {
        [f32::from(self.triggers[0]) / 255.0, f32::from(self.triggers[1]) / 255.0]
    }

    pub fn touchpad(&self) -> Option<Touchpad> {
        self.full.map(|full| Touchpad {
            fingers: full.touch_fingers,
            clicked: self.buttons.contains(Button::Touchpad),
            width: TOUCHPAD_WIDTH,
            height: TOUCHPAD_HEIGHT,
        })
    }
}

/// 0..=255 → [-1, 1], centre 128 (127,5 serait exact mais les manettes renvoient 128 au repos).
fn axis(raw: u8) -> f32 {
    ((f32::from(raw) - 128.0) / 127.0).clamp(-1.0, 1.0)
}

/// Analyse un rapport d'entrée. `data[0]` est l'identifiant de rapport.
pub fn parse_input(data: &[u8], transport: Transport, check_crc: bool) -> Result<RawInput, ParseError> {
    let id = *data.first().ok_or(ParseError::Empty)?;
    let too_short = || ParseError::TooShort { id, len: data.len() };
    match (transport, id) {
        (Transport::Usb, REPORT_ID_USB) => {
            let report = data.get(..USB_REPORT_SIZE).ok_or_else(too_short)?;
            Ok(parse_common(report, COMMON_OFFSET_USB, Some(USB_TOUCH_REPORTS)))
        }
        (Transport::Bluetooth, REPORT_ID_BT) => {
            let report = data.get(..BT_REPORT_SIZE).ok_or_else(too_short)?;
            if check_crc && !crc_is_valid(report, INPUT_CRC_SEED) {
                return Err(ParseError::BadCrc);
            }
            Ok(parse_common(report, COMMON_OFFSET_BT, Some(BT_TOUCH_REPORTS)))
        }
        (Transport::Bluetooth, REPORT_ID_USB) => {
            // Mode réduit : seuls sticks, boutons et gâchettes sont présents.
            let report = data.get(..BT_REDUCED_REPORT_SIZE).ok_or_else(too_short)?;
            Ok(parse_common(report, COMMON_OFFSET_USB, None))
        }
        _ => Err(ParseError::UnknownReport(id)),
    }
}

/// Déduit le transport d'après un premier rapport, quand l'énumération ne le donne pas.
pub fn guess_transport(data: &[u8]) -> Option<Transport> {
    match (data.first(), data.len()) {
        (Some(&REPORT_ID_BT), _) => Some(Transport::Bluetooth),
        (Some(&REPORT_ID_USB), len) if len >= USB_REPORT_SIZE => Some(Transport::Usb),
        (Some(&REPORT_ID_USB), len) if len >= BT_REDUCED_REPORT_SIZE => Some(Transport::Bluetooth),
        _ => None,
    }
}

/// `touch_reports == None` : rapport réduit, on s'arrête après les gâchettes.
/// Les tranches sont déjà validées à la bonne longueur par l'appelant.
fn parse_common(report: &[u8], offset: usize, touch_reports: Option<usize>) -> RawInput {
    let byte = |i: usize| report.get(offset + i).copied().unwrap_or(0);
    let le16 = |i: usize| i16::from_le_bytes([byte(i), byte(i + 1)]);

    let b0 = byte(C_BUTTONS);
    let b1 = byte(C_BUTTONS + 1);
    let b2 = byte(C_BUTTONS + 2);
    let mut buttons = hat_to_buttons(b0 & HAT_MASK);
    buttons.set(Button::West, b0 & (1 << 4) != 0);
    buttons.set(Button::South, b0 & (1 << 5) != 0);
    buttons.set(Button::East, b0 & (1 << 6) != 0);
    buttons.set(Button::North, b0 & (1 << 7) != 0);
    buttons.set(Button::L1, b1 & (1 << 0) != 0);
    buttons.set(Button::R1, b1 & (1 << 1) != 0);
    buttons.set(Button::L2, b1 & (1 << 2) != 0);
    buttons.set(Button::R2, b1 & (1 << 3) != 0);
    buttons.set(Button::Select, b1 & (1 << 4) != 0);
    buttons.set(Button::Start, b1 & (1 << 5) != 0);
    buttons.set(Button::L3, b1 & (1 << 6) != 0);
    buttons.set(Button::R3, b1 & (1 << 7) != 0);
    buttons.set(Button::Home, b2 & (1 << 0) != 0);
    buttons.set(Button::Touchpad, b2 & (1 << 1) != 0);

    let full = touch_reports.map(|max_reports| {
        let status0 = byte(C_STATUS);
        let status1 = byte(C_STATUS + 1);
        // Le driver Linux traite les touch_reports dans l'ordre : l'état final est le dernier.
        let count = usize::from(byte(C_NUM_TOUCH)).clamp(1, max_reports);
        let touch_base = C_TOUCH + (count - 1) * TOUCH_REPORT_SIZE;
        // +1 : on saute l'horodatage du touch_report.
        let finger = |n: usize| parse_finger(&|i| byte(touch_base + 1 + n * 4 + i));
        FullInput {
            sensor_timestamp: u16::from_le_bytes([byte(C_TIMESTAMP), byte(C_TIMESTAMP + 1)]),
            gyro: [le16(C_GYRO), le16(C_GYRO + 2), le16(C_GYRO + 4)],
            accel: [le16(C_ACCEL), le16(C_ACCEL + 2), le16(C_ACCEL + 4)],
            touch_fingers: [finger(0), finger(1)],
            battery: battery(status0),
            cable_connected: status0 & STATUS0_CABLE != 0,
            dongle_disconnected: status1 & STATUS1_DONGLE_DISCONNECTED != 0,
        }
    });

    RawInput {
        buttons,
        sticks: [byte(C_X), byte(C_Y), byte(C_RX), byte(C_RY)],
        triggers: [byte(C_Z), byte(C_RZ)],
        full,
    }
}

/// Croix directionnelle : 0 = haut, puis sens horaire, 8 = relâchée
/// (`ps_gamepad_hat_mapping` du driver Linux).
fn hat_to_buttons(hat: u8) -> Buttons {
    let mut buttons = Buttons::default();
    let (up, right, down, left) = match hat {
        0 => (true, false, false, false),
        1 => (true, true, false, false),
        2 => (false, true, false, false),
        3 => (false, true, true, false),
        4 => (false, false, true, false),
        5 => (false, false, true, true),
        6 => (false, false, false, true),
        7 => (true, false, false, true),
        _ => (false, false, false, false),
    };
    buttons.set(Button::DpadUp, up);
    buttons.set(Button::DpadRight, right);
    buttons.set(Button::DpadDown, down);
    buttons.set(Button::DpadLeft, left);
    buttons
}

fn parse_finger(byte: &dyn Fn(usize) -> u8) -> Finger {
    let contact = byte(0);
    let x_lo = byte(1);
    let mixed = byte(2);
    let y_hi = byte(3);
    Finger {
        active: contact & TOUCH_INACTIVE == 0,
        id: contact & 0x7F,
        // DS_TOUCH_POINT_X : bits 0..7 = x_lo, bits 8..11 = x_hi (4 bits de poids faible de l'octet 2)
        x: u16::from(x_lo) | (u16::from(mixed & 0x0F) << 8),
        // DS_TOUCH_POINT_Y : bits 0..3 = y_lo (4 bits de poids fort de l'octet 2), bits 4..11 = y_hi
        y: u16::from(mixed >> 4) | (u16::from(y_hi) << 4),
    }
}

/// Interprétation de `status[0]` selon `dualshock4_parse_report` (driver Linux) :
/// sans câble, 0..=10 = dizaines de pourcent ; avec câble, 0..=10 = en charge,
/// 11 = pleine, 14/15 = erreur de charge.
fn battery(status0: u8) -> Battery {
    let data = status0 & STATUS0_BATTERY;
    let midpoint = |d: u8| if d < 10 { d * 10 + 5 } else { 100 };
    if status0 & STATUS0_CABLE != 0 {
        match data {
            0..=10 => Battery { level: Some(midpoint(data)), status: BatteryStatus::Charging },
            BATTERY_FULL => Battery { level: Some(100), status: BatteryStatus::Full },
            _ => Battery { level: None, status: BatteryStatus::Unknown },
        }
    } else {
        Battery { level: Some(midpoint(data.min(10))), status: BatteryStatus::Discharging }
    }
}

/// CRC32 des rapports Bluetooth : CRC-32 IEEE sur `[seed] ++ report[..len-4]`,
/// stocké en little-endian dans les 4 derniers octets.
pub fn crc32_with_seed(seed: u8, data: &[u8]) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&[seed]);
    hasher.update(data);
    hasher.finalize()
}

pub fn crc_is_valid(report: &[u8], seed: u8) -> bool {
    let Some(split) = report.len().checked_sub(4) else {
        return false;
    };
    let (body, crc) = report.split_at(split);
    let expected = u32::from_le_bytes([crc[0], crc[1], crc[2], crc[3]]);
    crc32_with_seed(seed, body) == expected
}
