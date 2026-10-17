//! Parsing DS4 sur rapports bruts (fixtures générées par tests/fixtures/generate.py).

// Fichier de test : les helpers hors #[test] peuvent aussi échouer bruyamment.
#![allow(clippy::unwrap_used)]

use patchpad_core::ds4::calibration::{parse_calibration, CalibrationLayout, ImuCalibration};
use patchpad_core::ds4::output::{build_output_report, OutputState};
use patchpad_core::ds4::report::{
    crc_is_valid, guess_transport, parse_input, ParseError, Transport, INPUT_CRC_SEED, TOUCHPAD_HEIGHT, TOUCHPAD_WIDTH,
};
use patchpad_core::model::{BatteryStatus, Button, Buttons, Finger};

fn fixture(text: &str) -> Vec<u8> {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .flat_map(str::split_whitespace)
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}

const USB_FULL: &str = include_str!("fixtures/ds4_usb_full.hex");
const BT_FULL: &str = include_str!("fixtures/ds4_bt_full.hex");
const BT_NO_TOUCH: &str = include_str!("fixtures/ds4_bt_full_no_touch.hex");
const BT_REDUCED: &str = include_str!("fixtures/ds4_bt_reduced.hex");
const CAL_USB: &str = include_str!("fixtures/ds4_calibration_usb.hex");
const CAL_BT: &str = include_str!("fixtures/ds4_calibration_bt.hex");
const OUTPUT_BT: &str = include_str!("fixtures/ds4_output_bt.hex");

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn usb_full_report() {
    let data = fixture(USB_FULL);
    assert_eq!(data.len(), 64);
    let raw = parse_input(&data, Transport::Usb, true).unwrap();

    let expected: Buttons = [
        Button::South,
        Button::North,
        Button::L1,
        Button::L2,
        Button::Start,
        Button::R3,
        Button::Home,
        Button::Touchpad,
        Button::DpadRight,
    ]
    .into_iter()
    .collect();
    // Les bits 2..7 du 3e octet de boutons (compteur) doivent être ignorés.
    assert_eq!(raw.buttons, expected);

    assert_eq!(raw.left_stick().x, 0.0);
    assert_eq!(raw.left_stick().y, 0.0);
    assert_eq!(raw.right_stick().x, -1.0, "0x00 = gauche, borné à -1");
    assert_eq!(raw.right_stick().y, -1.0, "0xFF = bas, +Y vers le haut");
    assert!(approx(raw.triggers()[0], 64.0 / 255.0));
    assert_eq!(raw.triggers()[1], 1.0);

    let full = raw.full.unwrap();
    assert_eq!(full.sensor_timestamp, 0x1234);
    assert_eq!(full.gyro, [100, -200, 300]);
    assert_eq!(full.accel, [-50, 8192, 400]);
    assert!(full.cable_connected);
    assert_eq!(full.battery.level, Some(75));
    assert_eq!(full.battery.status, BatteryStatus::Charging);

    let touchpad = raw.touchpad().unwrap();
    assert!(touchpad.clicked);
    assert_eq!((touchpad.width, touchpad.height), (TOUCHPAD_WIDTH, TOUCHPAD_HEIGHT));
    assert_eq!(touchpad.fingers[0], Finger { active: true, id: 5, x: 1919, y: 941 });
    assert!(!touchpad.fingers[1].active);
    assert_eq!(touchpad.fingers[1].id, 6);
}

#[test]
fn bt_full_report_has_two_byte_offset_and_two_fingers() {
    let data = fixture(BT_FULL);
    assert_eq!(data.len(), 78);
    let raw = parse_input(&data, Transport::Bluetooth, true).unwrap();

    assert_eq!(raw.buttons, Buttons::default(), "croix relâchée (8) et aucun bouton");
    assert_eq!(raw.left_stick().x, -1.0);
    assert_eq!(raw.left_stick().y, -1.0);
    assert_eq!(raw.right_stick().x, 0.0);
    assert_eq!(raw.triggers(), [0.0, 0.0]);

    let full = raw.full.unwrap();
    assert_eq!(full.sensor_timestamp, 0xFFF0);
    assert_eq!(full.gyro, [-1, 0, 32767]);
    assert_eq!(full.accel, [0, 0, -8192]);
    assert!(!full.cable_connected);
    assert_eq!(full.battery.level, Some(35));
    assert_eq!(full.battery.status, BatteryStatus::Discharging);

    // Deux touch_reports : le dernier (le plus récent) est retenu.
    let touchpad = raw.touchpad().unwrap();
    assert!(!touchpad.clicked);
    assert_eq!(touchpad.fingers[0], Finger { active: true, id: 1, x: 100, y: 200 });
    assert_eq!(touchpad.fingers[1], Finger { active: true, id: 2, x: 1500, y: 700 });
}

#[test]
fn bt_report_without_fingers() {
    let raw = parse_input(&fixture(BT_NO_TOUCH), Transport::Bluetooth, true).unwrap();
    let touchpad = raw.touchpad().unwrap();
    assert!(touchpad.fingers.iter().all(|f| !f.active));
    // Les coordonnées restent lisibles même si le doigt est inactif.
    assert_eq!((touchpad.fingers[1].x, touchpad.fingers[1].y), (1500, 700));
}

#[test]
fn bt_crc_is_checked() {
    let mut data = fixture(BT_FULL);
    assert!(crc_is_valid(&data, INPUT_CRC_SEED));
    data[10] ^= 0x01;
    assert!(!crc_is_valid(&data, INPUT_CRC_SEED));
    assert_eq!(parse_input(&data, Transport::Bluetooth, true), Err(ParseError::BadCrc));
    // Vérification désactivée : le rapport est lu tel quel.
    assert!(parse_input(&data, Transport::Bluetooth, false).is_ok());
}

#[test]
fn bt_reduced_report() {
    let data = fixture(BT_REDUCED);
    assert_eq!(data.len(), 10);
    let raw = parse_input(&data, Transport::Bluetooth, true).unwrap();
    let expected: Buttons = [Button::West, Button::R1, Button::DpadUp, Button::DpadLeft].into_iter().collect();
    assert_eq!(raw.buttons, expected);
    assert!(approx(raw.right_stick().x, 64.0 / 127.0));
    assert!(approx(raw.right_stick().y, 64.0 / 127.0));
    assert!(approx(raw.triggers()[1], 128.0 / 255.0));
    assert!(raw.full.is_none());
    assert!(raw.touchpad().is_none());

    // Sous Windows, un rapport réduit peut arriver complété par des zéros.
    let mut padded = data.clone();
    padded.resize(78, 0);
    assert_eq!(parse_input(&padded, Transport::Bluetooth, true).unwrap(), raw);
}

#[test]
fn malformed_reports_are_rejected() {
    assert_eq!(parse_input(&[], Transport::Usb, true), Err(ParseError::Empty));
    let usb = fixture(USB_FULL);
    assert_eq!(parse_input(&usb[..20], Transport::Usb, true), Err(ParseError::TooShort { id: 0x01, len: 20 }));
    assert_eq!(parse_input(&fixture(BT_FULL), Transport::Usb, true), Err(ParseError::UnknownReport(0x11)));
    assert_eq!(parse_input(&[0x05, 0, 0], Transport::Bluetooth, true), Err(ParseError::UnknownReport(0x05)));
}

#[test]
fn transport_can_be_guessed_from_first_report() {
    assert_eq!(guess_transport(&fixture(USB_FULL)), Some(Transport::Usb));
    assert_eq!(guess_transport(&fixture(BT_FULL)), Some(Transport::Bluetooth));
    assert_eq!(guess_transport(&fixture(BT_REDUCED)), Some(Transport::Bluetooth));
    assert_eq!(guess_transport(&[0x01, 0x80]), None);
}

#[test]
fn battery_states() {
    let mut data = fixture(USB_FULL);
    // status[0] : 1 + 29 dans common.
    let status = 1 + 29;
    let battery = |data: &[u8]| parse_input(data, Transport::Usb, true).unwrap().full.unwrap().battery;

    data[status] = 0x10 | 11;
    assert_eq!((battery(&data).level, battery(&data).status), (Some(100), BatteryStatus::Full));
    data[status] = 0x10 | 10;
    assert_eq!((battery(&data).level, battery(&data).status), (Some(100), BatteryStatus::Charging));
    data[status] = 0x10 | 14;
    assert_eq!((battery(&data).level, battery(&data).status), (None, BatteryStatus::Unknown));
    data[status] = 0x00;
    assert_eq!((battery(&data).level, battery(&data).status), (Some(5), BatteryStatus::Discharging));
    data[status] = 0x0A;
    assert_eq!(battery(&data).level, Some(100));
}

fn expected_calibration_checks(cal: &ImuCalibration) {
    assert!(cal.factory);
    // Gyro : (speed+ + speed-) / (|plus - bias| + |minus - bias|), biais non appliqué (driver Linux).
    assert!(approx(cal.gyro[0].scale, 1080.0 / 17580.0));
    assert!(approx(cal.gyro[1].scale, 1080.0 / 17600.0));
    assert!(approx(cal.gyro[2].scale, 1080.0 / 17600.0));
    assert!(cal.gyro.iter().all(|g| g.bias == 0.0));
    // Accéléromètre : biais = plus - range/2, échelle = 2 / range.
    assert_eq!(cal.accel[0].bias, 10.0);
    assert_eq!(cal.accel[1].bias, 50.0);
    assert_eq!(cal.accel[2].bias, 15.0);
    assert!(approx(cal.accel[1].scale, 2.0 / 16400.0));

    let g = cal.accel_g([-50, 8192, 400]);
    assert!(approx(g[1], (8192.0 - 50.0) * 2.0 / 16400.0));
    let dps = cal.gyro_dps([100, -200, 300]);
    assert!(approx(dps[0], 100.0 * 1080.0 / 17580.0));
}

#[test]
fn usb_calibration() {
    let cal = parse_calibration(&fixture(CAL_USB), CalibrationLayout::Usb);
    expected_calibration_checks(&cal);
}

#[test]
fn bt_calibration_uses_interleaved_layout() {
    let data = fixture(CAL_BT);
    let cal = parse_calibration(&data, CalibrationLayout::BluetoothOrDongle);
    expected_calibration_checks(&cal);
    // Lu avec la disposition USB, le même rapport donne des valeurs différentes.
    assert_ne!(parse_calibration(&data, CalibrationLayout::Usb), cal);
}

#[test]
fn invalid_calibration_falls_back_to_nominal() {
    let nominal = ImuCalibration::default();
    assert!(!nominal.factory);
    assert!(approx(nominal.gyro_dps([16, 0, 0])[0], 1.0));
    assert!(approx(nominal.accel_g([0, 8192, 0])[1], 1.0));

    // Rapport rempli de zéros (adaptateur juste après connexion, selon SDL).
    let mut zeros = vec![0u8; 37];
    zeros[0] = 0x02;
    assert_eq!(parse_calibration(&zeros, CalibrationLayout::Usb), nominal);

    // CRC Bluetooth faux.
    let mut bt = fixture(CAL_BT);
    bt[20] ^= 0xFF;
    assert_eq!(parse_calibration(&bt, CalibrationLayout::BluetoothOrDongle), nominal);

    // Vitesse gyro nulle : échelle aberrante, rejetée.
    let mut usb = fixture(CAL_USB);
    usb[19..23].fill(0);
    assert_eq!(parse_calibration(&usb, CalibrationLayout::Usb), nominal);

    // Trop court.
    assert_eq!(parse_calibration(&[0x02, 1, 2, 3], CalibrationLayout::Usb), nominal);
}

#[test]
fn usb_output_report() {
    let state = OutputState { motor_left: 0x80, motor_right: 0x40, lightbar: [0x11, 0x22, 0x33] };
    let report = build_output_report(Transport::Usb, state);
    assert_eq!(report.len(), 32);
    assert_eq!(&report[..9], &[0x05, 0x03, 0x00, 0x00, 0x40, 0x80, 0x11, 0x22, 0x33]);
    assert!(report[9..].iter().all(|b| *b == 0));
}

#[test]
fn bt_output_report_with_crc32() {
    let state = OutputState { motor_left: 0x80, motor_right: 0x40, lightbar: [0x11, 0x22, 0x33] };
    let report = build_output_report(Transport::Bluetooth, state);
    assert_eq!(report, fixture(OUTPUT_BT), "CRC32 attendu calculé par zlib (graine 0xA2)");
    assert!(crc_is_valid(&report, 0xA2));
}
