//! Cœur de PatchPad, sans dépendance UI : backends de manettes, état normalisé et analyses.
//!
//! - [`ds4`] : DualShock 4 via hidapi, parsing des rapports bruts (prioritaire pour Sony).
//! - [`gilrs_backend`] : Xbox et autres manettes via gilrs.
//! - [`analysis`] : drift, zone morte, circularité, gâchettes, fréquence, orientation, journal.
//! - [`report`] : suivi de ce qui a été testé et export HTML / PDF du rapport.
//! - [`manager`] : thread d'acquisition et instantanés limités à ~60 par seconde.

pub mod analysis;
pub mod backend;
pub mod ds4;
pub mod gilrs_backend;
pub mod manager;
pub mod model;
pub mod report;
#[cfg(target_os = "macos")]
pub mod xbox_mac;

pub use backend::{BackendEvent, ControllerBackend, Notice, NoticeCode, OutputError, Rumble};
pub use manager::{Manager, Snapshot};
