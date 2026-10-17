//! macOS : détection des manettes Xbox One / Series branchées en USB.
//!
//! En USB, ces manettes parlent GIP (interface vendeur 0xFF/0x47/0xD0), pas HID, et macOS
//! n'a pas de pilote : elles sont invisibles pour gilrs et hidapi. On les repère sur le bus
//! USB pour afficher un message clair (« utilisez le Bluetooth ») plutôt qu'une absence muette.

use std::time::{Duration, Instant};

use nusb::MaybeFuture;

use crate::backend::{BackendEvent, ControllerBackend, Notice, NoticeCode, OutputError, Rumble};
use crate::gilrs_backend::MICROSOFT_VID;

const SCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Interface GIP : classe vendeur, sous-classe 0x47, protocole 0xD0.
const GIP_INTERFACE: (u8, u8, u8) = (0xFF, 0x47, 0xD0);

/// Manettes et adaptateurs Xbox One / Series connus (au cas où les interfaces ne sont pas
/// énumérées).
const KNOWN_PIDS: [u16; 10] = [0x02D1, 0x02DD, 0x02E3, 0x02EA, 0x0B00, 0x0B12, 0x0B0A, 0x02E6, 0x02FE, 0x0B11];

#[derive(Default)]
pub struct XboxUsbProbe {
    found: Vec<String>,
    last_scan: Option<Instant>,
}

impl XboxUsbProbe {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ControllerBackend for XboxUsbProbe {
    fn name(&self) -> &'static str {
        "usb-probe"
    }

    fn poll(&mut self, now: Instant, _out: &mut Vec<BackendEvent>) {
        if self.last_scan.is_some_and(|t| now.duration_since(t) < SCAN_INTERVAL) {
            return;
        }
        self.last_scan = Some(now);
        let Ok(devices) = nusb::list_devices().wait() else { return };
        self.found = devices
            .filter(|d| d.vendor_id() == MICROSOFT_VID)
            .filter(|d| {
                KNOWN_PIDS.contains(&d.product_id())
                    || d.interfaces().any(|i| (i.class(), i.subclass(), i.protocol()) == GIP_INTERFACE)
            })
            .map(|d| d.product_string().unwrap_or("Xbox").to_owned())
            .collect();
    }

    fn notices(&self) -> Vec<Notice> {
        self.found.iter().map(|name| Notice { code: NoticeCode::XboxUsbMacUnsupported, detail: name.clone() }).collect()
    }

    fn owns(&self, _id: &str) -> bool {
        false
    }

    fn rumble(&mut self, _id: &str, _rumble: Rumble, _now: Instant) -> Result<(), OutputError> {
        Err(OutputError::Unsupported)
    }
}
