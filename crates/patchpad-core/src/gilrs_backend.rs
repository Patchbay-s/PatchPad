//! Backend générique via gilrs : manettes Xbox et toute autre manette reconnue par l'OS
//! (XInput sous Windows, evdev sous Linux, IOKit sous macOS).
//!
//! gilrs ne publie qu'en cas de changement : la « fréquence » mesurée pour ces manettes est
//! celle des changements d'état, pas celle des rapports matériels.

use std::collections::HashMap;
use std::time::Instant;

use gilrs::ff::{BaseEffect, BaseEffectType, Effect, EffectBuilder, Repeat, Replay, Ticks};
use gilrs::{Axis, Button as GButton, EventType, Gamepad, GamepadId, Gilrs, GilrsBuilder, PowerInfo};

use crate::backend::{BackendEvent, ControllerBackend, Notice, NoticeCode, OutputError, Rumble};
use crate::model::{
    Battery, BatteryStatus, Button, Buttons, Capabilities, Connection, ControllerInfo, ControllerState, Family, Stick,
    Support,
};

pub const MICROSOFT_VID: u16 = 0x045E;

/// Correspondance boutons gilrs → boutons normalisés.
const BUTTON_MAP: [(GButton, Button); 17] = [
    (GButton::South, Button::South),
    (GButton::East, Button::East),
    (GButton::West, Button::West),
    (GButton::North, Button::North),
    (GButton::LeftTrigger, Button::L1),
    (GButton::RightTrigger, Button::R1),
    (GButton::Select, Button::Select),
    (GButton::Start, Button::Start),
    (GButton::LeftThumb, Button::L3),
    (GButton::RightThumb, Button::R3),
    (GButton::Mode, Button::Home),
    (GButton::DPadUp, Button::DpadUp),
    (GButton::DPadDown, Button::DpadDown),
    (GButton::DPadLeft, Button::DpadLeft),
    (GButton::DPadRight, Button::DpadRight),
    // Gâchettes numériques, présentes sur certaines manettes génériques.
    (GButton::LeftTrigger2, Button::L2),
    (GButton::RightTrigger2, Button::R2),
];

pub struct GilrsBackend {
    gilrs: Result<Gilrs, String>,
    pads: HashMap<GamepadId, Pad>,
    excluded: Vec<(u16, u16)>,
    initialized: bool,
}

struct Pad {
    info: ControllerInfo,
    vid_pid: Option<(u16, u16)>,
    /// Masquée car déjà ouverte par un backend prioritaire (DS4 via hidapi).
    hidden: bool,
    effect: Option<(Effect, Instant)>,
}

impl GilrsBackend {
    pub fn new() -> Self {
        // Sans filtres par défaut (zone morte, anti-jitter) : on veut les valeurs brutes pour
        // diagnostiquer le drift.
        let gilrs = GilrsBuilder::new().with_default_filters(false).build().map_err(|e| e.to_string());
        GilrsBackend { gilrs, pads: HashMap::new(), excluded: Vec::new(), initialized: false }
    }

    fn register(&mut self, id: GamepadId, out: &mut Vec<BackendEvent>) {
        let Ok(gilrs) = &self.gilrs else { return };
        let Some(gamepad) = gilrs.connected_gamepad(id) else { return };
        let vid_pid = gamepad.vendor_id().zip(gamepad.product_id());
        let hidden = vid_pid.is_some_and(|v| self.excluded.contains(&v));
        let info = pad_info(&gamepad);
        if !hidden {
            out.push(BackendEvent::Connected(info.clone()));
        }
        self.pads.insert(id, Pad { info, vid_pid, hidden, effect: None });
    }
}

impl Default for GilrsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerBackend for GilrsBackend {
    fn name(&self) -> &'static str {
        "gilrs"
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<BackendEvent>) {
        if !self.initialized {
            self.initialized = true;
            let ids: Vec<GamepadId> = match &self.gilrs {
                Ok(gilrs) => gilrs.gamepads().map(|(id, _)| id).collect(),
                Err(_) => Vec::new(),
            };
            for id in ids {
                self.register(id, out);
            }
        }

        let mut changed: Vec<GamepadId> = Vec::new();
        let mut events = Vec::new();
        if let Ok(gilrs) = &mut self.gilrs {
            while let Some(event) = gilrs.next_event() {
                events.push((event.id, event.event));
            }
        }
        for (id, event) in events {
            match event {
                EventType::Connected => {
                    if !self.pads.contains_key(&id) {
                        self.register(id, out);
                    }
                }
                EventType::Disconnected => {
                    if let Some(pad) = self.pads.remove(&id) {
                        if !pad.hidden {
                            out.push(BackendEvent::Disconnected(pad.info.id));
                        }
                    }
                }
                EventType::Dropped | EventType::ForceFeedbackEffectCompleted => {}
                _ => {
                    if !self.pads.contains_key(&id) {
                        self.register(id, out);
                    }
                    if !changed.contains(&id) {
                        changed.push(id);
                    }
                }
            }
        }

        // Exclusion dynamique : la DS4 peut être ouverte (ou perdue) par hidapi à tout moment.
        for (id, pad) in &mut self.pads {
            let hidden = pad.vid_pid.is_some_and(|v| self.excluded.contains(&v));
            if hidden != pad.hidden {
                pad.hidden = hidden;
                if hidden {
                    out.push(BackendEvent::Disconnected(pad.info.id.clone()));
                } else {
                    out.push(BackendEvent::Connected(pad.info.clone()));
                    changed.push(*id);
                }
            }
            if pad.effect.as_ref().is_some_and(|(_, until)| now >= *until) {
                pad.effect = None;
            }
        }

        let Ok(gilrs) = &self.gilrs else { return };
        for id in changed {
            let Some(pad) = self.pads.get(&id) else { continue };
            if pad.hidden {
                continue;
            }
            if let Some(gamepad) = gilrs.connected_gamepad(id) {
                out.push(BackendEvent::Input { id: pad.info.id.clone(), state: pad_state(&gamepad), at: now });
            }
        }
    }

    fn notices(&self) -> Vec<Notice> {
        match &self.gilrs {
            Ok(_) => Vec::new(),
            Err(error) => vec![Notice { code: NoticeCode::GilrsUnavailable, detail: error.clone() }],
        }
    }

    fn set_excluded_devices(&mut self, devices: &[(u16, u16)]) {
        devices.clone_into(&mut self.excluded);
    }

    fn owns(&self, id: &str) -> bool {
        self.pads.values().any(|p| !p.hidden && p.info.id == id)
    }

    fn rumble(&mut self, id: &str, rumble: Rumble, now: Instant) -> Result<(), OutputError> {
        let Ok(gilrs) = &mut self.gilrs else { return Err(OutputError::Unsupported) };
        let (gamepad_id, pad) = self
            .pads
            .iter_mut()
            .find(|(_, p)| !p.hidden && p.info.id == id)
            .ok_or_else(|| OutputError::UnknownController(id.to_owned()))?;
        if pad.info.capabilities.rumble != Support::Supported {
            return Err(OutputError::Unsupported);
        }
        let ms = u32::try_from(rumble.duration.as_millis()).unwrap_or(u32::MAX);
        let ticks = Ticks::from_ms(ms);
        let scheduling = Replay { play_for: ticks, ..Default::default() };
        let effect = EffectBuilder::new()
            .add_effect(BaseEffect {
                kind: BaseEffectType::Strong { magnitude: unit_to_u16(rumble.strong) },
                scheduling,
                ..Default::default()
            })
            .add_effect(BaseEffect {
                kind: BaseEffectType::Weak { magnitude: unit_to_u16(rumble.weak) },
                scheduling,
                ..Default::default()
            })
            .repeat(Repeat::For(ticks))
            .gamepads(&[*gamepad_id])
            .finish(gilrs)
            .map_err(|e| OutputError::Write(e.to_string()))?;
        effect.play().map_err(|e| OutputError::Write(e.to_string()))?;
        // L'effet s'arrête s'il est détruit : on le garde jusqu'à la fin.
        pad.effect = Some((effect, now + rumble.duration));
        Ok(())
    }
}

fn unit_to_u16(value: f32) -> u16 {
    let scaled = (value.clamp(0.0, 1.0) * f32::from(u16::MAX)).round();
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let magnitude = scaled as u16;
    magnitude
}

fn pad_info(gamepad: &Gamepad<'_>) -> ControllerInfo {
    let vid = gamepad.vendor_id();
    let family = if vid == Some(MICROSOFT_VID) { Family::Xbox } else { Family::Generic };
    let rumble = if gamepad.is_ff_supported() { Support::Supported } else { Support::UnsupportedOs };
    ControllerInfo {
        id: format!("gilrs:{}", usize::from(gamepad.id())),
        name: gamepad.name().to_owned(),
        family,
        backend: "gilrs",
        connection: connection(gamepad, family),
        vendor_id: vid,
        product_id: gamepad.product_id(),
        capabilities: Capabilities {
            touchpad: false,
            imu: false,
            rumble,
            lightbar: Support::NoHardware,
            analog_triggers: true,
        },
        report_mode: None,
        imu_factory_calibration: None,
        dongle_idle: false,
    }
}

/// Type de connexion, au mieux : gilrs ne l'expose pas directement.
/// - Linux : le bus evdev est encodé dans les 2 premiers octets de l'UUID (format SDL).
/// - macOS : l'UUID annonce toujours l'USB ; une Xbox ne peut y être qu'en Bluetooth.
/// - Windows (XInput) : seul « filaire / sans fil » est connu, via la batterie.
fn connection(gamepad: &Gamepad<'_>, family: Family) -> Connection {
    if cfg!(target_os = "macos") {
        return if family == Family::Xbox { Connection::Bluetooth } else { Connection::Unknown };
    }
    if cfg!(target_os = "linux") {
        let uuid = gamepad.uuid();
        match u16::from_le_bytes([uuid[0], uuid[1]]) {
            0x03 => return Connection::Usb,
            0x05 => return Connection::Bluetooth,
            _ => {}
        }
    }
    match gamepad.power_info() {
        PowerInfo::Wired => Connection::Usb,
        PowerInfo::Discharging(_) | PowerInfo::Charging(_) | PowerInfo::Charged => Connection::Wireless,
        PowerInfo::Unknown => Connection::Unknown,
    }
}

/// Seuil au-delà duquel un axe de croix compte comme appuyé.
const DPAD_AXIS_THRESHOLD: f32 = 0.5;

/// Croix directionnelle remontée en axes : macOS (hat switch HID, émulé en `DPadX`/`DPadY`)
/// et Linux (`ABS_HAT0X`/`ABS_HAT0Y`). gilrs ne les convertit en boutons que via son filtre
/// par défaut `axis_dpad_to_button`, désactivé ici pour garder les sticks bruts.
/// Convention gilrs : +X = droite, +Y = haut.
fn dpad_from_axes(x: f32, y: f32) -> Vec<Button> {
    let mut pressed = Vec::new();
    if y >= DPAD_AXIS_THRESHOLD {
        pressed.push(Button::DpadUp);
    }
    if y <= -DPAD_AXIS_THRESHOLD {
        pressed.push(Button::DpadDown);
    }
    if x <= -DPAD_AXIS_THRESHOLD {
        pressed.push(Button::DpadLeft);
    }
    if x >= DPAD_AXIS_THRESHOLD {
        pressed.push(Button::DpadRight);
    }
    pressed
}

fn pad_state(gamepad: &Gamepad<'_>) -> ControllerState {
    let mut buttons: Buttons = BUTTON_MAP.iter().filter(|(g, _)| gamepad.is_pressed(*g)).map(|(_, b)| *b).collect();
    for button in dpad_from_axes(gamepad.value(Axis::DPadX), gamepad.value(Axis::DPadY)) {
        buttons.set(button, true);
    }
    let trigger = |button: GButton, axis: Axis| {
        gamepad
            .button_data(button)
            .map(|d| d.value())
            .or_else(|| gamepad.axis_data(axis).map(|d| (d.value() + 1.0) / 2.0))
            .unwrap_or(0.0)
            .clamp(0.0, 1.0)
    };
    let battery = match gamepad.power_info() {
        PowerInfo::Unknown => None,
        PowerInfo::Wired => Some(Battery { level: None, status: BatteryStatus::Wired }),
        PowerInfo::Discharging(level) => Some(Battery { level: Some(level), status: BatteryStatus::Discharging }),
        PowerInfo::Charging(level) => Some(Battery { level: Some(level), status: BatteryStatus::Charging }),
        PowerInfo::Charged => Some(Battery { level: Some(100), status: BatteryStatus::Full }),
    };
    ControllerState {
        buttons,
        left_stick: Stick { x: gamepad.value(Axis::LeftStickX), y: gamepad.value(Axis::LeftStickY) },
        right_stick: Stick { x: gamepad.value(Axis::RightStickX), y: gamepad.value(Axis::RightStickY) },
        left_trigger: trigger(GButton::LeftTrigger2, Axis::LeftZ),
        right_trigger: trigger(GButton::RightTrigger2, Axis::RightZ),
        touchpad: None,
        imu: None,
        battery,
        cable_connected: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpad_axes_map_to_buttons() {
        assert!(dpad_from_axes(0.0, 0.0).is_empty());
        assert_eq!(dpad_from_axes(0.0, 1.0), vec![Button::DpadUp]);
        assert_eq!(dpad_from_axes(0.0, -1.0), vec![Button::DpadDown]);
        assert_eq!(dpad_from_axes(-1.0, 0.0), vec![Button::DpadLeft]);
        assert_eq!(dpad_from_axes(1.0, 0.0), vec![Button::DpadRight]);
        // Diagonale haut-droite.
        assert_eq!(dpad_from_axes(1.0, 1.0), vec![Button::DpadUp, Button::DpadRight]);
        // Bruit sous le seuil.
        assert!(dpad_from_axes(0.2, -0.3).is_empty());
    }
}
