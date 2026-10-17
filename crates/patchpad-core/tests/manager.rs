//! Gestionnaire et session, avec des backends factices.

// Fichier de test : les helpers hors #[test] peuvent aussi échouer bruyamment.
#![allow(clippy::unwrap_used)]

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use patchpad_core::analysis::log::LogKind;
use patchpad_core::backend::{BackendEvent, ControllerBackend, OutputError, Rumble};
use patchpad_core::manager::{Manager, Session, Snapshot};
use patchpad_core::model::{
    Button, Buttons, Capabilities, Connection, ControllerInfo, ControllerState, Family, Imu, Stick, Support,
};

fn info(id: &str) -> ControllerInfo {
    ControllerInfo {
        id: id.to_owned(),
        name: format!("Pad {id}"),
        family: Family::Generic,
        backend: "fake",
        connection: Connection::Usb,
        vendor_id: Some(0x1234),
        product_id: Some(0x0001),
        capabilities: Capabilities {
            touchpad: false,
            imu: true,
            rumble: Support::Supported,
            lightbar: Support::NoHardware,
            analog_triggers: true,
        },
        report_mode: None,
        imu_factory_calibration: None,
        dongle_idle: false,
    }
}

fn input(id: &str, buttons: &[Button], at: Instant) -> BackendEvent {
    let state = ControllerState {
        buttons: buttons.iter().copied().collect(),
        left_stick: Stick { x: 0.1, y: 0.0 },
        imu: Some(Imu { gyro: [2.0, 0.0, 0.0], accel: [0.0, 1.0, 0.0] }),
        ..Default::default()
    };
    BackendEvent::Input { id: id.to_owned(), state, at }
}

#[test]
fn session_tracks_state_log_and_rate() {
    let start = Instant::now();
    let mut session = Session::new(start);
    session.apply(BackendEvent::Connected(info("a")), start);
    for i in 0..250u32 {
        let at = start + Duration::from_millis(4) * i;
        let buttons: &[Button] = if (100..150).contains(&i) { &[Button::South] } else { &[] };
        session.apply(input("a", buttons, at), at);
    }
    let now = start + Duration::from_millis(996);
    let snapshot = session.snapshot(now, Vec::new());
    assert_eq!(snapshot.controllers.len(), 1);
    let view = &snapshot.controllers[0];
    assert!((view.rate.hz - 250.0).abs() < 1.0, "{:?}", view.rate);
    assert_eq!(view.left_stick.rest_offset.map(|o| (o * 100.0).round()), Some(10.0));
    assert!(view.orientation.is_some());

    let kinds: Vec<_> = snapshot.new_log.iter().map(|e| (e.kind, e.button)).collect();
    assert_eq!(
        kinds,
        vec![
            (LogKind::Connected, None),
            (LogKind::Pressed, Some(Button::South)),
            (LogKind::Released, Some(Button::South))
        ]
    );
    assert_eq!(snapshot.new_log[1].t_ms, 400);
    // Le journal n'est envoyé qu'une fois.
    assert!(session.snapshot(now, Vec::new()).new_log.is_empty());

    // Recalibrage du zéro : le gyroscope affiché passe à ~0 une fois la mesure terminée.
    assert!(session.recalibrate_imu("a"));
    for i in 250..600u32 {
        let at = start + Duration::from_millis(4) * i;
        session.apply(input("a", &[], at), at);
    }
    let view = &session.snapshot(start + Duration::from_millis(2400), Vec::new()).controllers[0];
    assert!(!view.imu_zeroing);
    assert!(view.state.imu.unwrap().gyro[0].abs() < 1e-4);

    session.reset_analysis(None);
    let view = &session.snapshot(now, Vec::new()).controllers[0];
    assert_eq!(view.left_stick.rest, None);

    session.apply(BackendEvent::Disconnected("a".into()), now);
    let snapshot = session.snapshot(now, Vec::new());
    assert!(snapshot.controllers.is_empty());
    assert_eq!(snapshot.new_log.last().map(|e| e.kind), Some(LogKind::Disconnected));
}

#[test]
fn drift_threshold_is_applied() {
    let start = Instant::now();
    let mut session = Session::new(start);
    session.apply(BackendEvent::Connected(info("a")), start);
    for i in 0..200u32 {
        let at = start + Duration::from_millis(4) * i;
        session.apply(input("a", &[], at), at);
    }
    let now = start + Duration::from_millis(800);
    use patchpad_core::analysis::stick::DriftVerdict;
    assert_eq!(session.snapshot(now, Vec::new()).controllers[0].left_stick.drift, DriftVerdict::Drift);
    session.set_drift_threshold(0.2);
    let snapshot = session.snapshot(now, Vec::new());
    assert_eq!(snapshot.drift_threshold, 0.2);
    assert_eq!(snapshot.controllers[0].left_stick.drift, DriftVerdict::Ok);
}

/// Backend factice : une manette, VID/PID 1234:0001, enregistre les vibrations demandées.
struct FakeBackend {
    id: &'static str,
    claim: bool,
    excluded: Vec<(u16, u16)>,
    announced: bool,
    rumbles: Arc<Mutex<Vec<(String, f32)>>>,
}

impl ControllerBackend for FakeBackend {
    fn name(&self) -> &'static str {
        "fake"
    }

    fn poll(&mut self, now: Instant, out: &mut Vec<BackendEvent>) {
        let hidden = self.excluded.contains(&(0x1234, 0x0001));
        if !self.announced && !hidden {
            self.announced = true;
            out.push(BackendEvent::Connected(info(self.id)));
            out.push(input(self.id, &[Button::Start], now));
        }
    }

    fn claimed_devices(&self) -> Vec<(u16, u16)> {
        if self.claim {
            vec![(0x1234, 0x0001)]
        } else {
            Vec::new()
        }
    }

    fn set_excluded_devices(&mut self, devices: &[(u16, u16)]) {
        self.excluded = devices.to_vec();
    }

    fn owns(&self, id: &str) -> bool {
        self.announced && id == self.id
    }

    fn rumble(&mut self, id: &str, rumble: Rumble, _now: Instant) -> Result<(), OutputError> {
        self.rumbles.lock().unwrap().push((id.to_owned(), rumble.strong));
        Ok(())
    }
}

#[test]
fn manager_routes_commands_and_deduplicates() {
    let rumbles = Arc::new(Mutex::new(Vec::new()));
    let backend_rumbles = Arc::clone(&rumbles);
    let (tx, rx) = mpsc::channel::<Snapshot>();
    let mut manager = Manager::with_backends(
        Box::new(move || {
            let make = |id, claim| -> Box<dyn ControllerBackend> {
                Box::new(FakeBackend {
                    id,
                    claim,
                    excluded: Vec::new(),
                    announced: false,
                    rumbles: Arc::clone(&backend_rumbles),
                })
            };
            // Le premier backend « ouvre » la manette : le second doit l'ignorer.
            vec![make("primary", true), make("secondary", false)]
        }),
        Box::new(move |snapshot| {
            tx.send(snapshot).ok();
        }),
    )
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    let snapshot = loop {
        let snapshot = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if !snapshot.controllers.is_empty() || Instant::now() > deadline {
            break snapshot;
        }
    };
    let ids: Vec<_> = snapshot.controllers.iter().map(|c| c.info.id.as_str()).collect();
    assert_eq!(ids, vec!["primary"]);
    assert_eq!(snapshot.controllers[0].state.buttons, [Button::Start].into_iter().collect::<Buttons>());

    let rumble = Rumble { strong: 0.5, weak: 0.25, duration: Duration::from_millis(100) };
    manager.rumble("primary", rumble).unwrap();
    assert_eq!(*rumbles.lock().unwrap(), vec![("primary".to_owned(), 0.5)]);
    assert_eq!(manager.rumble("nope", rumble), Err(OutputError::UnknownController("nope".into())));
    assert_eq!(manager.set_lightbar("primary", [1, 2, 3]), Err(OutputError::Unsupported));

    // Les instantanés sont limités à ~60 par seconde.
    while rx.try_recv().is_ok() {}
    std::thread::sleep(Duration::from_millis(300));
    let count = rx.try_iter().count();
    assert!(count <= 20, "{count} instantanés en 300 ms");

    manager.shutdown();
}
