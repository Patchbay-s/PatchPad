//! Rapports de sortie de la DualShock 4 : vibration et barre lumineuse.
//!
//! Disposition reprise de `struct dualshock4_output_report_common/_usb/_bt` et de
//! `dualshock4_output_worker` (driver Linux `hid-playstation.c`), confirmée par
//! `HIDAPI_DriverPS4_UpdateEffects` (SDL : données d'effet à l'offset 4 en USB, 6 en BT) :
//!
//! ```text
//! common : valid_flag0, valid_flag1, reserved, motor_right, motor_left,
//!          lightbar_red, lightbar_green, lightbar_blue, blink_on, blink_off
//! usb (32) : report_id 0x05, common, reserved[21]
//! bt  (78) : report_id 0x11, hw_control, audio_control, common, reserved[61], crc32 (le32)
//! ```
//!
//! Le driver Linux envoie toujours vibration et barre lumineuse ensemble (certaines manettes
//! annulent l'une quand on n'envoie que l'autre) : on fait de même.

use super::report::{crc32_with_seed, Transport};

pub const OUTPUT_REPORT_USB: u8 = 0x05;
pub const OUTPUT_REPORT_USB_SIZE: usize = 32;
pub const OUTPUT_REPORT_BT: u8 = 0x11;
pub const OUTPUT_REPORT_BT_SIZE: usize = 78;
/// `PS_OUTPUT_CRC32_SEED`
pub const OUTPUT_CRC_SEED: u8 = 0xA2;

const VALID_FLAG0_MOTOR: u8 = 0x01;
const VALID_FLAG0_LED: u8 = 0x02;
/// `DS4_OUTPUT_HWCTL_HID | DS4_OUTPUT_HWCTL_CRC32`
const HWCTL_HID_CRC32: u8 = 0x80 | 0x40;
/// Intervalle de rapport Bluetooth en ms (`DS4_BT_DEFAULT_POLL_INTERVAL_MS`, comme l'USB).
const BT_POLL_INTERVAL_MS: u8 = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutputState {
    /// Moteur gauche, basse fréquence (« fort »).
    pub motor_left: u8,
    /// Moteur droit, haute fréquence (« faible »).
    pub motor_right: u8,
    pub lightbar: [u8; 3],
}

pub fn build_output_report(transport: Transport, state: OutputState) -> Vec<u8> {
    let (mut report, common) = match transport {
        Transport::Usb => {
            let mut report = vec![0u8; OUTPUT_REPORT_USB_SIZE];
            report[0] = OUTPUT_REPORT_USB;
            (report, 1)
        }
        Transport::Bluetooth => {
            let mut report = vec![0u8; OUTPUT_REPORT_BT_SIZE];
            report[0] = OUTPUT_REPORT_BT;
            report[1] = HWCTL_HID_CRC32 | BT_POLL_INTERVAL_MS;
            (report, 3)
        }
    };
    report[common] = VALID_FLAG0_MOTOR | VALID_FLAG0_LED;
    report[common + 3] = state.motor_right;
    report[common + 4] = state.motor_left;
    report[common + 5..common + 8].copy_from_slice(&state.lightbar);

    if transport == Transport::Bluetooth {
        let body = OUTPUT_REPORT_BT_SIZE - 4;
        let crc = crc32_with_seed(OUTPUT_CRC_SEED, &report[..body]);
        report[body..].copy_from_slice(&crc.to_le_bytes());
    }
    report
}
