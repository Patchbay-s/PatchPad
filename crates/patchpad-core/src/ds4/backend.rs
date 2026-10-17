//! Backend DualShock 4 via hidapi : énumération, ouverture, lecture des rapports bruts.

use std::collections::HashMap;
use std::ffi::CString;
use std::time::{Duration, Instant};

use hidapi::{BusType, HidApi, HidDevice};

use super::calibration::{
    parse_calibration, CalibrationLayout, ImuCalibration, FEATURE_CALIBRATION_BT, FEATURE_CALIBRATION_BT_SIZE,
    FEATURE_CALIBRATION_USB, FEATURE_CALIBRATION_USB_SIZE,
};
use super::output::{build_output_report, OutputState};
use super::report::{guess_transport, parse_input, ParseError, RawInput, Transport, BT_REPORT_SIZE};
use super::{is_ds4, Model, SONY_VID};
use crate::backend::{unit_to_u8, BackendEvent, ControllerBackend, Notice, NoticeCode, OutputError, Rumble};
use crate::model::{Capabilities, Connection, ControllerInfo, ControllerState, Family, Imu, ReportMode, Support};

const SCAN_INTERVAL: Duration = Duration::from_secs(1);
/// Délai avant de réessayer un périphérique dont l'ouverture a échoué.
const RETRY_OPEN: Duration = Duration::from_secs(5);
/// Si la manette reste en mode réduit, on redemande la calibration (max 3 fois).
const REDUCED_RETRY: Duration = Duration::from_secs(1);
const MAX_FULL_MODE_ATTEMPTS: u8 = 3;
/// Après autant d'échecs CRC consécutifs, on considère que l'OS altère les rapports et on
/// arrête de vérifier (plutôt que de tout rejeter).
const MAX_CRC_FAILURES: u32 = 20;
/// Nombre max de rapports lus par appel à `poll`.
const MAX_READS_PER_POLL: usize = 128;

pub struct Ds4Backend {
    api: Result<HidApi, String>,
    devices: Vec<Ds4Device>,
    failed: HashMap<CString, (Instant, Notice)>,
    last_scan: Option<Instant>,
}

struct Ds4Device {
    path: CString,
    device: HidDevice,
    info: ControllerInfo,
    model: Model,
    /// `None` tant que l'énumération ne l'a pas donné et qu'aucun rapport n'est arrivé.
    transport: Option<Transport>,
    calibration: ImuCalibration,
    output: OutputState,
    rumble_until: Option<Instant>,
    crc_failures: u32,
    check_crc: bool,
    full_mode_attempts: u8,
    last_full_mode_attempt: Instant,
    reduced_notice: bool,
}

impl Ds4Backend {
    pub fn new() -> Self {
        // Pas d'énumération globale : `scan` n'énumère que les périphériques Sony.
        HidApi::disable_device_discovery();
        let api = HidApi::new().map_err(|e| e.to_string());
        Ds4Backend { api, devices: Vec::new(), failed: HashMap::new(), last_scan: None }
    }

    fn scan(&mut self, now: Instant, out: &mut Vec<BackendEvent>) {
        let Ok(api) = &mut self.api else { return };
        // Énumération ciblée sur Sony uniquement.
        api.reset_devices().ok();
        if api.add_devices(SONY_VID, 0).is_err() {
            return;
        }

        let mut present = Vec::new();
        for info in api.device_list() {
            if !is_ds4(info.vendor_id(), info.product_id()) {
                continue;
            }
            // Sous Windows et macOS, ne garder que la collection « Game Pad » (page 0x01, usage 0x05).
            if info.usage_page() != 0 && (info.usage_page(), info.usage()) != (0x01, 0x05) {
                continue;
            }
            let path = info.path().to_owned();
            present.push(path.clone());
            if self.devices.iter().any(|d| d.path == path) {
                continue;
            }
            if let Some((at, _)) = self.failed.get(&path) {
                if now.duration_since(*at) < RETRY_OPEN {
                    continue;
                }
            }
            let model = Model::from_pid(info.product_id());
            let transport = match info.bus_type() {
                BusType::Usb => Some(Transport::Usb),
                BusType::Bluetooth => Some(Transport::Bluetooth),
                _ if model == Model::Dongle => Some(Transport::Usb),
                _ => None,
            };
            match api.open_path(&path) {
                Ok(device) => {
                    self.failed.remove(&path);
                    let dev = Ds4Device::open(path, device, model, transport, info.product_id(), now);
                    out.push(BackendEvent::Connected(dev.info.clone()));
                    self.devices.push(dev);
                }
                Err(error) => {
                    let notice = open_failure_notice(&path, &error.to_string());
                    self.failed.insert(path, (now, notice));
                }
            }
        }

        // Disparus de l'énumération (Bluetooth sous macOS notamment, sans erreur de lecture).
        self.devices.retain(|d| {
            let keep = present.contains(&d.path);
            if !keep {
                out.push(BackendEvent::Disconnected(d.info.id.clone()));
            }
            keep
        });
        self.failed.retain(|path, _| present.contains(path));
    }
}

impl Default for Ds4Backend {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerBackend for Ds4Backend {
    fn name(&self) -> &'static str {
        "hidapi"
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<BackendEvent>) {
        if self.last_scan.is_none_or(|t| now.duration_since(t) >= SCAN_INTERVAL) {
            self.last_scan = Some(now);
            self.scan(now, out);
        }
        self.devices.retain_mut(|device| match device.read_all(now, out) {
            Ok(()) => true,
            Err(()) => {
                out.push(BackendEvent::Disconnected(device.info.id.clone()));
                false
            }
        });
    }

    fn notices(&self) -> Vec<Notice> {
        let mut notices: Vec<Notice> = Vec::new();
        if let Err(error) = &self.api {
            notices.push(Notice { code: NoticeCode::HidUnavailable, detail: error.clone() });
        }
        notices.extend(self.failed.values().map(|(_, notice)| notice.clone()));
        notices.extend(
            self.devices
                .iter()
                .filter(|d| d.reduced_notice)
                .map(|d| Notice { code: NoticeCode::Ds4ReducedMode, detail: d.info.name.clone() }),
        );
        notices
    }

    fn claimed_devices(&self) -> Vec<(u16, u16)> {
        self.devices.iter().map(|d| (SONY_VID, d.model.pid())).collect()
    }

    fn owns(&self, id: &str) -> bool {
        self.devices.iter().any(|d| d.info.id == id)
    }

    fn rumble(&mut self, id: &str, rumble: Rumble, now: Instant) -> Result<(), OutputError> {
        let device = self.device_mut(id)?;
        device.output.motor_left = unit_to_u8(rumble.strong);
        device.output.motor_right = unit_to_u8(rumble.weak);
        device.rumble_until = Some(now + rumble.duration);
        device.send_output()
    }

    fn set_lightbar(&mut self, id: &str, rgb: [u8; 3]) -> Result<(), OutputError> {
        let device = self.device_mut(id)?;
        device.output.lightbar = rgb;
        device.send_output()
    }
}

impl Ds4Backend {
    fn device_mut(&mut self, id: &str) -> Result<&mut Ds4Device, OutputError> {
        self.devices.iter_mut().find(|d| d.info.id == id).ok_or_else(|| OutputError::UnknownController(id.to_owned()))
    }
}

impl Ds4Device {
    fn open(
        path: CString,
        device: HidDevice,
        model: Model,
        transport: Option<Transport>,
        pid: u16,
        now: Instant,
    ) -> Self {
        device.set_blocking_mode(false).ok();
        let connection = match (model, transport) {
            (Model::Dongle, _) => Connection::Dongle,
            (_, Some(Transport::Usb)) => Connection::Usb,
            (_, Some(Transport::Bluetooth)) => Connection::Bluetooth,
            (_, None) => Connection::Unknown,
        };
        let info = ControllerInfo {
            id: format!("ds4:{}", path.to_string_lossy()),
            name: model.name().to_owned(),
            family: Family::DualShock4,
            backend: "hidapi",
            connection,
            vendor_id: Some(SONY_VID),
            product_id: Some(pid),
            capabilities: Capabilities {
                touchpad: true,
                imu: true,
                rumble: Support::Supported,
                lightbar: Support::Supported,
                analog_triggers: true,
            },
            report_mode: None,
            imu_factory_calibration: None,
            dongle_idle: false,
        };
        let mut dev = Ds4Device {
            path,
            device,
            info,
            model,
            transport,
            calibration: ImuCalibration::default(),
            output: OutputState::default(),
            rumble_until: None,
            crc_failures: 0,
            check_crc: true,
            full_mode_attempts: 0,
            last_full_mode_attempt: now,
            reduced_notice: false,
        };
        if transport.is_some() {
            dev.load_calibration(now);
        }
        dev
    }

    /// Lit la calibration. En Bluetooth, la lecture du feature report 0x05 fait passer la
    /// manette des rapports réduits 0x01 aux rapports complets 0x11 (driver Linux et SDL ;
    /// SDL lit d'abord 0x02, ce que l'on reproduit).
    fn load_calibration(&mut self, now: Instant) {
        let Some(transport) = self.transport else { return };
        self.full_mode_attempts = self.full_mode_attempts.saturating_add(1);
        self.last_full_mode_attempt = now;
        let mut buf = [0u8; FEATURE_CALIBRATION_BT_SIZE];
        buf[0] = FEATURE_CALIBRATION_USB;
        let usb = self.device.get_feature_report(&mut buf[..FEATURE_CALIBRATION_USB_SIZE]);
        let result = match transport {
            Transport::Usb => usb.map(|len| buf[..len].to_vec()),
            Transport::Bluetooth => {
                let mut bt = [0u8; FEATURE_CALIBRATION_BT_SIZE];
                bt[0] = FEATURE_CALIBRATION_BT;
                self.device.get_feature_report(&mut bt).map(|len| bt[..len].to_vec())
            }
        };
        let layout = CalibrationLayout::for_device(transport, self.model == Model::Dongle);
        self.calibration = match result {
            Ok(data) => parse_calibration(&data, layout),
            Err(_) => ImuCalibration::default(),
        };
        self.info.imu_factory_calibration = Some(self.calibration.factory);
    }

    fn read_all(&mut self, now: Instant, out: &mut Vec<BackendEvent>) -> Result<(), ()> {
        if self.rumble_until.is_some_and(|t| now >= t) {
            self.rumble_until = None;
            self.output.motor_left = 0;
            self.output.motor_right = 0;
            self.send_output().ok();
        }

        let mut buf = [0u8; BT_REPORT_SIZE + 16];
        for _ in 0..MAX_READS_PER_POLL {
            let len = self.device.read_timeout(&mut buf, 0).map_err(|_| ())?;
            if len == 0 {
                break;
            }
            let data = &buf[..len];
            if self.transport.is_none() {
                self.transport = guess_transport(data);
                if let Some(transport) = self.transport {
                    self.info.connection = match transport {
                        Transport::Usb => Connection::Usb,
                        Transport::Bluetooth => Connection::Bluetooth,
                    };
                    self.load_calibration(now);
                    out.push(BackendEvent::InfoChanged(self.info.clone()));
                }
            }
            let Some(transport) = self.transport else { continue };
            match parse_input(data, transport, self.check_crc) {
                Ok(raw) => {
                    self.crc_failures = 0;
                    self.handle_report(transport, &raw, now, out);
                }
                Err(ParseError::BadCrc) => {
                    self.crc_failures += 1;
                    if self.crc_failures >= MAX_CRC_FAILURES {
                        self.check_crc = false;
                    }
                }
                Err(_) => {}
            }
        }
        Ok(())
    }

    fn handle_report(&mut self, transport: Transport, raw: &RawInput, now: Instant, out: &mut Vec<BackendEvent>) {
        let mode = if raw.full.is_some() { ReportMode::Full } else { ReportMode::Reduced };
        let dongle_idle = self.model == Model::Dongle && raw.full.is_some_and(|f| f.dongle_disconnected);
        if self.info.report_mode != Some(mode) || self.info.dongle_idle != dongle_idle {
            let reconnected = self.info.dongle_idle && !dongle_idle;
            self.info.report_mode = Some(mode);
            self.info.dongle_idle = dongle_idle;
            if reconnected {
                // Nouvelle manette appairée sur l'adaptateur : nouvelle calibration.
                self.load_calibration(now);
            }
            out.push(BackendEvent::InfoChanged(self.info.clone()));
        }

        if mode == ReportMode::Reduced && transport == Transport::Bluetooth {
            if self.full_mode_attempts < MAX_FULL_MODE_ATTEMPTS {
                if now.duration_since(self.last_full_mode_attempt) >= REDUCED_RETRY {
                    self.load_calibration(now);
                }
            } else {
                self.reduced_notice = true;
            }
        } else {
            self.reduced_notice = false;
        }

        let triggers = raw.triggers();
        let state = ControllerState {
            buttons: raw.buttons,
            left_stick: raw.left_stick(),
            right_stick: raw.right_stick(),
            left_trigger: triggers[0],
            right_trigger: triggers[1],
            touchpad: raw.touchpad(),
            imu: raw.full.map(|full| Imu {
                gyro: self.calibration.gyro_dps(full.gyro),
                accel: self.calibration.accel_g(full.accel),
            }),
            battery: raw.full.map(|full| full.battery),
            cable_connected: raw.full.map(|full| full.cable_connected),
        };
        out.push(BackendEvent::Input { id: self.info.id.clone(), state, at: now });
    }

    fn send_output(&mut self) -> Result<(), OutputError> {
        let transport = self.transport.ok_or(OutputError::Unsupported)?;
        let report = build_output_report(transport, self.output);
        self.device.write(&report).map(|_| ()).map_err(|e| OutputError::Write(e.to_string()))
    }
}

impl Drop for Ds4Device {
    /// Ne pas laisser une manette vibrer après la fermeture de l'application.
    fn drop(&mut self) {
        if self.rumble_until.is_some() {
            self.output.motor_left = 0;
            self.output.motor_right = 0;
            self.send_output().ok();
        }
    }
}

/// Classe une erreur d'ouverture : refus d'accès Linux (règle udev manquante), refus macOS,
/// ou autre.
fn open_failure_notice(path: &CString, error: &str) -> Notice {
    let lower = error.to_lowercase();
    let detail = format!("{} : {error}", path.to_string_lossy());
    let denied = lower.contains("permission denied") || lower.contains("eacces") || lower.contains("os error 13");
    let code = if cfg!(target_os = "linux") && denied {
        NoticeCode::HidrawPermission
    } else if cfg!(target_os = "macos")
        && (denied
            || lower.contains("not permitted")
            || lower.contains("privilege")
            || lower.contains("0xe00002e2")
            || lower.contains("0xe00002c1"))
    {
        NoticeCode::MacHidDenied
    } else {
        NoticeCode::HidOpenFailed
    };
    Notice { code, detail }
}
