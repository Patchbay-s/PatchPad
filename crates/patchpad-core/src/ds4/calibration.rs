//! Calibration IMU de la DualShock 4 (feature report 0x02 en USB, 0x05 en Bluetooth).
//!
//! Disposition et formules reprises de `dualshock4_get_calibration_data` du driver Linux
//! (`drivers/hid/hid-playstation.c`) et de `HIDAPI_DriverPS4_LoadOfficialCalibrationData`
//! (SDL, `SDL_hidapi_ps4.c`), qui concordent :
//!
//! ```text
//! [1..7]   gyro pitch/yaw/roll bias (le16)
//! [7..19]  USB          : pitch+ pitch- yaw+ yaw- roll+ roll-
//!          BT / dongle  : pitch+ yaw+ roll+ pitch- yaw- roll-
//! [19..23] gyro speed+ speed-
//! [23..35] accel x+ x- y+ y- z+ z-
//! ```
//!
//! Gyro : °/s = brut × (speed+ + speed-) / (|plus − bias| + |minus − bias|). Le driver Linux
//! n'applique pas de biais au gyro (`bias = 0`), SDL soustrait `bias` : on suit Linux et le
//! bouton « recalibrer le zéro » corrige le décalage résiduel.
//! Accéléromètre : g = (brut − (plus − range/2)) × 2 / range, avec range = plus − minus.

use super::report::{crc_is_valid, Transport};

/// `DS4_FEATURE_REPORT_CALIBRATION` (USB)
pub const FEATURE_CALIBRATION_USB: u8 = 0x02;
pub const FEATURE_CALIBRATION_USB_SIZE: usize = 37;
/// `DS4_FEATURE_REPORT_CALIBRATION_BT` : sa lecture fait aussi passer la manette en rapports 0x11.
pub const FEATURE_CALIBRATION_BT: u8 = 0x05;
pub const FEATURE_CALIBRATION_BT_SIZE: usize = 41;
/// `PS_FEATURE_CRC32_SEED`
pub const FEATURE_CRC_SEED: u8 = 0xA3;

/// Résolutions nominales du capteur (SDL : `gyro_denominator = 16`, `accel_denominator = 8192`),
/// utilisées si la calibration est absente ou incohérente.
const GYRO_LSB_PER_DPS: f32 = 16.0;
const ACCEL_LSB_PER_G: f32 = 8192.0;

/// Disposition des valeurs gyro « plus/minus » : le driver Linux et SDL utilisent la
/// disposition Bluetooth aussi pour l'adaptateur sans fil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationLayout {
    Usb,
    BluetoothOrDongle,
}

impl CalibrationLayout {
    pub fn for_device(transport: Transport, is_dongle: bool) -> Self {
        match transport {
            Transport::Usb if !is_dongle => CalibrationLayout::Usb,
            _ => CalibrationLayout::BluetoothOrDongle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisCalibration {
    pub bias: f32,
    /// Unités physiques par LSB.
    pub scale: f32,
}

impl AxisCalibration {
    pub fn apply(self, raw: i16) -> f32 {
        (f32::from(raw) - self.bias) * self.scale
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImuCalibration {
    pub gyro: [AxisCalibration; 3],
    pub accel: [AxisCalibration; 3],
    /// `true` si les valeurs viennent de la manette, `false` pour les valeurs nominales.
    pub factory: bool,
}

impl Default for ImuCalibration {
    fn default() -> Self {
        let gyro = AxisCalibration { bias: 0.0, scale: 1.0 / GYRO_LSB_PER_DPS };
        let accel = AxisCalibration { bias: 0.0, scale: 1.0 / ACCEL_LSB_PER_G };
        ImuCalibration { gyro: [gyro; 3], accel: [accel; 3], factory: false }
    }
}

impl ImuCalibration {
    pub fn gyro_dps(&self, raw: [i16; 3]) -> [f32; 3] {
        [self.gyro[0].apply(raw[0]), self.gyro[1].apply(raw[1]), self.gyro[2].apply(raw[2])]
    }

    pub fn accel_g(&self, raw: [i16; 3]) -> [f32; 3] {
        [self.accel[0].apply(raw[0]), self.accel[1].apply(raw[1]), self.accel[2].apply(raw[2])]
    }
}

/// Analyse un feature report de calibration (`buf[0]` = identifiant de rapport).
/// Retourne la calibration nominale si les données sont absentes, nulles ou incohérentes.
pub fn parse_calibration(buf: &[u8], layout: CalibrationLayout) -> ImuCalibration {
    if buf.len() < 35 || buf.iter().skip(1).all(|b| *b == 0) {
        // SDL : « dans certains cas ce rapport ne contient que des zéros » (adaptateur).
        return ImuCalibration::default();
    }
    // Un rapport Bluetooth complet porte un CRC : s'il est faux, les données ne sont pas fiables.
    if buf.first() == Some(&FEATURE_CALIBRATION_BT)
        && buf.len() >= FEATURE_CALIBRATION_BT_SIZE
        && !crc_is_valid(&buf[..FEATURE_CALIBRATION_BT_SIZE], FEATURE_CRC_SEED)
    {
        return ImuCalibration::default();
    }

    let v = |i: usize| f32::from(i16::from_le_bytes([buf[i], buf[i + 1]]));
    let bias = [v(1), v(3), v(5)];
    let (plus, minus) = match layout {
        CalibrationLayout::Usb => ([v(7), v(11), v(15)], [v(9), v(13), v(17)]),
        CalibrationLayout::BluetoothOrDongle => ([v(7), v(9), v(11)], [v(13), v(15), v(17)]),
    };
    let speed_2x = v(19) + v(21);

    let nominal = ImuCalibration::default();
    let mut calibration = ImuCalibration { factory: true, ..nominal };
    for axis in 0..3 {
        let denom = (plus[axis] - bias[axis]).abs() + (minus[axis] - bias[axis]).abs();
        calibration.gyro[axis] = AxisCalibration { bias: 0.0, scale: speed_2x / denom };

        let acc_plus = v(23 + axis * 4);
        let acc_minus = v(25 + axis * 4);
        let range_2g = acc_plus - acc_minus;
        // Division entière dans le driver : on reproduit l'arrondi vers zéro.
        let acc_bias = acc_plus - (range_2g / 2.0).trunc();
        calibration.accel[axis] = AxisCalibration { bias: acc_bias, scale: 2.0 / range_2g };
    }

    // Garde-fou repris de SDL : biais > 1024 ou échelle à plus de 50 % du nominal → on ignore
    // (manettes non officielles, données corrompues). Couvre aussi les divisions par zéro.
    let sane = |cal: AxisCalibration, nominal: AxisCalibration| {
        let ratio = cal.scale / nominal.scale;
        cal.scale.is_finite() && cal.bias.abs() <= 1024.0 && (ratio - 1.0).abs() <= 0.5
    };
    let all_sane =
        (0..3).all(|i| sane(calibration.gyro[i], nominal.gyro[i]) && sane(calibration.accel[i], nominal.accel[i]));
    if all_sane {
        calibration
    } else {
        nominal
    }
}
