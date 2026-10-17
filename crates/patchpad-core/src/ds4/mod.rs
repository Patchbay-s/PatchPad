//! Sony DualShock 4 : parsing des rapports HID bruts et backend hidapi.

pub mod backend;
pub mod calibration;
pub mod output;
pub mod report;

pub const SONY_VID: u16 = 0x054C;
pub const PID_DS4_V1: u16 = 0x05C4;
pub const PID_DS4_V2: u16 = 0x09CC;
pub const PID_DS4_DONGLE: u16 = 0x0BA0;

pub fn is_ds4(vid: u16, pid: u16) -> bool {
    vid == SONY_VID && matches!(pid, PID_DS4_V1 | PID_DS4_V2 | PID_DS4_DONGLE)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    V1,
    V2,
    Dongle,
}

impl Model {
    pub fn from_pid(pid: u16) -> Self {
        match pid {
            PID_DS4_V1 => Model::V1,
            PID_DS4_DONGLE => Model::Dongle,
            _ => Model::V2,
        }
    }

    pub fn pid(self) -> u16 {
        match self {
            Model::V1 => PID_DS4_V1,
            Model::V2 => PID_DS4_V2,
            Model::Dongle => PID_DS4_DONGLE,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Model::V1 => "DualShock 4 (v1)",
            Model::V2 => "DualShock 4 (v2)",
            Model::Dongle => "DualShock 4 USB Wireless Adaptor",
        }
    }
}
