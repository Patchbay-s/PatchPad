//! Orientation approximative à partir de l'IMU : filtre complémentaire.
//!
//! Axes de la DS4 (convention SDL) : X vers la droite, Y vers le haut (face du pavé),
//! Z vers le joueur ; gyro x = tangage, y = lacet, z = roulis (règle de la main droite).
//! Tangage et roulis sont corrigés par la gravité ; le lacet n'a pas de référence absolue
//! et dérive lentement.

use serde::Serialize;

/// Part du gyroscope dans le filtre (le reste vient de l'accéléromètre).
const GYRO_WEIGHT: f32 = 0.98;
/// Durée de la mesure du zéro du gyroscope, en échantillons (~1 s à 250 Hz).
const ZERO_SAMPLES: u32 = 250;
/// Intervalle max accepté entre deux échantillons (au-delà : pause, on n'intègre pas).
const MAX_DT: f32 = 0.1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orientation {
    /// Degrés, positif quand l'avant de la manette se lève.
    pub pitch: f32,
    /// Degrés, positif quand le côté droit se lève.
    pub roll: f32,
    /// Degrés, intégration du gyroscope seule.
    pub yaw: f32,
}

#[derive(Debug, Clone, Default)]
pub struct OrientationFilter {
    orientation: Orientation,
    gyro_offset: [f32; 3],
    initialized: bool,
    zeroing: Option<([f32; 3], u32)>,
}

impl OrientationFilter {
    /// Lance la mesure du zéro du gyroscope (manette immobile) et remet l'orientation à zéro.
    pub fn recalibrate(&mut self) {
        self.zeroing = Some(([0.0; 3], 0));
        self.initialized = false;
        self.orientation.yaw = 0.0;
    }

    pub fn is_calibrating(&self) -> bool {
        self.zeroing.is_some()
    }

    pub fn gyro_offset(&self) -> [f32; 3] {
        self.gyro_offset
    }

    /// Gyroscope corrigé du zéro mesuré.
    pub fn corrected(&self, gyro: [f32; 3]) -> [f32; 3] {
        [gyro[0] - self.gyro_offset[0], gyro[1] - self.gyro_offset[1], gyro[2] - self.gyro_offset[2]]
    }

    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// `gyro` en °/s (non corrigé), `accel` en g, `dt` en secondes.
    pub fn update(&mut self, gyro: [f32; 3], accel: [f32; 3], dt: f32) {
        if let Some((sum, count)) = &mut self.zeroing {
            for (s, g) in sum.iter_mut().zip(gyro) {
                *s += g;
            }
            *count += 1;
            if *count >= ZERO_SAMPLES {
                #[allow(clippy::cast_precision_loss)]
                let n = *count as f32;
                self.gyro_offset = [sum[0] / n, sum[1] / n, sum[2] / n];
                self.zeroing = None;
            }
        }

        let [ax, ay, az] = accel;
        let pitch_acc = (-az).atan2(ay).to_degrees();
        let roll_acc = ax.atan2(ay).to_degrees();
        if !self.initialized {
            self.orientation.pitch = pitch_acc;
            self.orientation.roll = roll_acc;
            self.initialized = true;
            return;
        }
        if !(dt > 0.0 && dt <= MAX_DT) {
            return;
        }
        let [gx, gy, gz] = self.corrected(gyro);
        let o = &mut self.orientation;
        o.pitch = blend(o.pitch + gx * dt, pitch_acc);
        o.roll = blend(o.roll + gz * dt, roll_acc);
        o.yaw = wrap(o.yaw + gy * dt);
    }
}

/// Mélange gyro/accéléromètre en tenant compte du passage ±180°.
fn blend(gyro_angle: f32, acc_angle: f32) -> f32 {
    let delta = wrap(acc_angle - gyro_angle);
    wrap(gyro_angle + (1.0 - GYRO_WEIGHT) * delta)
}

fn wrap(angle: f32) -> f32 {
    (angle + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.004;

    #[test]
    fn flat_controller_is_level() {
        let mut f = OrientationFilter::default();
        for _ in 0..100 {
            f.update([0.0; 3], [0.0, 1.0, 0.0], DT);
        }
        let o = f.orientation();
        assert!(o.pitch.abs() < 1e-3 && o.roll.abs() < 1e-3 && o.yaw.abs() < 1e-3);
    }

    #[test]
    fn accelerometer_gives_pitch_and_roll() {
        let (s, c) = 30f32.to_radians().sin_cos();
        let mut f = OrientationFilter::default();
        // Avant levé de 30° : la gravité passe en partie sur -Z.
        f.update([0.0; 3], [0.0, c, -s], DT);
        assert!((f.orientation().pitch - 30.0).abs() < 1e-3);

        let mut f = OrientationFilter::default();
        // Côté droit levé de 30° : la gravité passe en partie sur +X.
        f.update([0.0; 3], [s, c, 0.0], DT);
        assert!((f.orientation().roll - 30.0).abs() < 1e-3);
    }

    #[test]
    fn filter_converges_to_accelerometer_tilt() {
        let (s, c) = 20f32.to_radians().sin_cos();
        let mut f = OrientationFilter::default();
        f.update([0.0; 3], [0.0, 1.0, 0.0], DT);
        for _ in 0..2000 {
            f.update([0.0; 3], [0.0, c, -s], DT);
        }
        assert!((f.orientation().pitch - 20.0).abs() < 0.1, "{:?}", f.orientation());
    }

    #[test]
    fn gyro_integrates_yaw() {
        let mut f = OrientationFilter::default();
        f.update([0.0; 3], [0.0, 1.0, 0.0], DT);
        // 90 °/s pendant 1 s.
        for _ in 0..250 {
            f.update([0.0, 90.0, 0.0], [0.0, 1.0, 0.0], DT);
        }
        assert!((f.orientation().yaw - 90.0).abs() < 0.5, "{:?}", f.orientation());
    }

    #[test]
    fn recalibration_removes_gyro_offset() {
        let mut f = OrientationFilter::default();
        f.recalibrate();
        assert!(f.is_calibrating());
        for _ in 0..ZERO_SAMPLES {
            f.update([1.5, -2.0, 0.5], [0.0, 1.0, 0.0], DT);
        }
        assert!(!f.is_calibrating());
        let offset = f.gyro_offset();
        assert!((offset[0] - 1.5).abs() < 1e-4 && (offset[1] + 2.0).abs() < 1e-4 && (offset[2] - 0.5).abs() < 1e-4);
        assert_eq!(f.corrected([1.5, -2.0, 0.5]), [0.0, 0.0, 0.0]);
        let yaw = f.orientation().yaw;
        for _ in 0..250 {
            f.update([1.5, -2.0, 0.5], [0.0, 1.0, 0.0], DT);
        }
        assert!((f.orientation().yaw - yaw).abs() < 1e-3);
    }

    #[test]
    fn angles_wrap() {
        assert_eq!(wrap(190.0), -170.0);
        assert_eq!(wrap(-190.0), 170.0);
        assert!((blend(179.0, -179.0) - 179.04).abs() < 1e-3);
    }
}
