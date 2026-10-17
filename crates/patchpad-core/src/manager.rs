//! Gestionnaire : interroge les backends dans un thread dédié, alimente les analyses et
//! publie des instantanés limités en fréquence (~60 par seconde).

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::analysis::log::{EventLog, LogEntry, LogKind};
use crate::analysis::orientation::{Orientation, OrientationFilter};
use crate::analysis::rate::{RateMeter, RateStats};
use crate::analysis::stick::{StickAnalyzer, StickReport};
use crate::analysis::trigger::{TriggerAnalyzer, TriggerReport};
use crate::backend::{BackendEvent, ControllerBackend, Notice, OutputError, Rumble};
use crate::model::{ControllerInfo, ControllerState, Imu};
use crate::report::{Confirmation, OutputItem, ReportData, ReportProgress};

/// Intervalle minimal entre deux instantanés (~60 fps).
pub const EMIT_INTERVAL: Duration = Duration::from_micros(16_667);
/// Même sans changement, un instantané part à cet intervalle (fréquences qui retombent…).
const IDLE_EMIT_INTERVAL: Duration = Duration::from_millis(500);
const POLL_SLEEP: Duration = Duration::from_millis(1);
pub const DEFAULT_DRIFT_THRESHOLD: f32 = 0.05;
const LOG_CAPACITY: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerView {
    pub info: ControllerInfo,
    /// État courant ; le gyroscope y est corrigé du zéro mesuré.
    pub state: ControllerState,
    pub rate: RateStats,
    pub left_stick: StickReport,
    pub right_stick: StickReport,
    pub left_trigger: Option<TriggerReport>,
    pub right_trigger: Option<TriggerReport>,
    pub orientation: Option<Orientation>,
    pub imu_zeroing: bool,
    /// Ce qui a été testé depuis le début du rapport (connexion ou « nouveau test »).
    pub report: ReportProgress,
    /// Secondes écoulées depuis le début du rapport.
    pub report_elapsed_s: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub controllers: Vec<ControllerView>,
    pub notices: Vec<Notice>,
    /// Entrées de journal apparues depuis l'instantané précédent.
    pub new_log: Vec<LogEntry>,
    pub drift_threshold: f32,
}

struct Tracker {
    info: ControllerInfo,
    state: ControllerState,
    rate: RateMeter,
    left: StickAnalyzer,
    right: StickAnalyzer,
    triggers: [TriggerAnalyzer; 2],
    orientation: OrientationFilter,
    last_imu: Option<Instant>,
    report: ReportProgress,
    report_start: Instant,
}

impl Tracker {
    fn new(info: ControllerInfo, now: Instant) -> Self {
        let report = new_report(&info);
        Tracker {
            info,
            state: ControllerState::default(),
            rate: RateMeter::default(),
            left: StickAnalyzer::default(),
            right: StickAnalyzer::default(),
            triggers: [TriggerAnalyzer::default(); 2],
            orientation: OrientationFilter::default(),
            last_imu: None,
            report,
            report_start: now,
        }
    }

    /// Nouveau test : mesures et suivi du rapport repartent de zéro.
    fn start_report(&mut self, now: Instant) {
        self.reset_analysis();
        self.report = new_report(&self.info);
        self.report_start = now;
    }

    fn reset_analysis(&mut self) {
        self.left = StickAnalyzer::default();
        self.right = StickAnalyzer::default();
        self.triggers = [TriggerAnalyzer::default(); 2];
    }

    fn view(&mut self, now: Instant, drift_threshold: f32) -> ControllerView {
        let mut state = self.state;
        if let Some(imu) = &mut state.imu {
            imu.gyro = self.orientation.corrected(imu.gyro);
        }
        ControllerView {
            info: self.info.clone(),
            state,
            rate: self.rate.stats(now),
            left_stick: self.left.report(drift_threshold),
            right_stick: self.right.report(drift_threshold),
            left_trigger: self.triggers[0].report(),
            right_trigger: self.triggers[1].report(),
            orientation: self.last_imu.map(|_| self.orientation.orientation()),
            imu_zeroing: self.orientation.is_calibrating(),
            report: self.report.clone(),
            report_elapsed_s: now.saturating_duration_since(self.report_start).as_secs_f32(),
        }
    }
}

fn new_report(info: &ControllerInfo) -> ReportProgress {
    ReportProgress::new(info.family, info.capabilities.touchpad, info.capabilities.imu)
}

/// État de l'analyse, indépendant des threads et des backends (testable directement).
pub struct Session {
    start: Instant,
    trackers: Vec<Tracker>,
    log: EventLog,
    emitted_seq: u64,
    drift_threshold: f32,
}

impl Session {
    pub fn new(start: Instant) -> Self {
        Session {
            start,
            trackers: Vec::new(),
            log: EventLog::new(LOG_CAPACITY),
            emitted_seq: 0,
            drift_threshold: DEFAULT_DRIFT_THRESHOLD,
        }
    }

    fn t_ms(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.start).as_millis()).unwrap_or(u64::MAX)
    }

    pub fn apply(&mut self, event: BackendEvent, now: Instant) {
        match event {
            BackendEvent::Connected(info) | BackendEvent::InfoChanged(info) => {
                if let Some(tracker) = self.trackers.iter_mut().find(|t| t.info.id == info.id) {
                    tracker.info = info;
                } else {
                    let t_ms = self.t_ms(now);
                    self.log.push(t_ms, &info.id, &info.name, LogKind::Connected, None);
                    self.trackers.push(Tracker::new(info, now));
                }
            }
            BackendEvent::Disconnected(id) => {
                if let Some(index) = self.trackers.iter().position(|t| t.info.id == id) {
                    let tracker = self.trackers.remove(index);
                    let t_ms = self.t_ms(now);
                    self.log.push(t_ms, &id, &tracker.info.name, LogKind::Disconnected, None);
                }
            }
            BackendEvent::Input { id, state, at } => {
                let t_ms = self.t_ms(at);
                let Some(tracker) = self.trackers.iter_mut().find(|t| t.info.id == id) else { return };
                self.log.push_diff(t_ms, &id, &tracker.info.name, tracker.state.buttons, state.buttons);
                tracker.rate.record(at);
                tracker.left.record(at, state.left_stick);
                tracker.right.record(at, state.right_stick);
                tracker.triggers[0].record(state.left_trigger);
                tracker.triggers[1].record(state.right_trigger);
                if let Some(Imu { gyro, accel }) = state.imu {
                    let dt = tracker.last_imu.map_or(0.0, |last| at.saturating_duration_since(last).as_secs_f32());
                    tracker.orientation.update(gyro, accel, dt);
                    tracker.last_imu = Some(at);
                }
                let mut corrected = state;
                if let Some(imu) = &mut corrected.imu {
                    imu.gyro = tracker.orientation.corrected(imu.gyro);
                }
                let report_ms =
                    u64::try_from(at.saturating_duration_since(tracker.report_start).as_millis()).unwrap_or(u64::MAX);
                tracker.report.record(&corrected, report_ms);
                tracker.state = state;
            }
        }
    }

    pub fn set_drift_threshold(&mut self, threshold: f32) {
        self.drift_threshold = threshold.clamp(0.0, 1.0);
    }

    /// `None` : réinitialise toutes les manettes.
    pub fn reset_analysis(&mut self, id: Option<&str>) {
        for tracker in &mut self.trackers {
            if id.is_none_or(|id| tracker.info.id == id) {
                tracker.reset_analysis();
            }
        }
    }

    pub fn start_report(&mut self, id: &str, now: Instant) -> bool {
        match self.trackers.iter_mut().find(|t| t.info.id == id) {
            Some(tracker) => {
                tracker.start_report(now);
                true
            }
            None => false,
        }
    }

    pub fn confirm_output(&mut self, id: &str, item: OutputItem, result: Confirmation) -> bool {
        match self.trackers.iter_mut().find(|t| t.info.id == id) {
            Some(tracker) => {
                tracker.report.confirm(item, result);
                true
            }
            None => false,
        }
    }

    /// Données figées pour l'export du rapport.
    pub fn report_data(&mut self, id: &str, now: Instant) -> Option<ReportData> {
        let threshold = self.drift_threshold;
        let tracker = self.trackers.iter_mut().find(|t| t.info.id == id)?;
        let view = tracker.view(now, threshold);
        let duration_s = view.report_elapsed_s;
        Some(ReportData { view, drift_threshold: threshold, duration_s })
    }

    pub fn recalibrate_imu(&mut self, id: &str) -> bool {
        match self.trackers.iter_mut().find(|t| t.info.id == id) {
            Some(tracker) => {
                tracker.orientation.recalibrate();
                true
            }
            None => false,
        }
    }

    pub fn snapshot(&mut self, now: Instant, notices: Vec<Notice>) -> Snapshot {
        let new_log = self.log.since(self.emitted_seq);
        self.emitted_seq = self.log.last_seq();
        let threshold = self.drift_threshold;
        Snapshot {
            controllers: self.trackers.iter_mut().map(|t| t.view(now, threshold)).collect(),
            notices,
            new_log,
            drift_threshold: threshold,
        }
    }
}

enum Command {
    Rumble { id: String, rumble: Rumble, reply: Sender<Result<(), OutputError>> },
    Lightbar { id: String, rgb: [u8; 3], reply: Sender<Result<(), OutputError>> },
    ResetAnalysis(Option<String>),
    RecalibrateImu(String),
    SetDriftThreshold(f32),
    StartReport(String),
    ConfirmOutput { id: String, item: OutputItem, result: Confirmation },
    ReportData { id: String, reply: Sender<Option<ReportData>> },
    Shutdown,
}

pub type BackendFactory = Box<dyn FnOnce() -> Vec<Box<dyn ControllerBackend>> + Send>;
pub type SnapshotSink = Box<dyn FnMut(Snapshot) + Send>;

/// Backends par défaut, par ordre de priorité.
pub fn default_backends() -> Vec<Box<dyn ControllerBackend>> {
    let mut backends: Vec<Box<dyn ControllerBackend>> =
        vec![Box::new(crate::ds4::backend::Ds4Backend::new()), Box::new(crate::gilrs_backend::GilrsBackend::new())];
    #[cfg(target_os = "macos")]
    backends.push(Box::new(crate::xbox_mac::XboxUsbProbe::new()));
    backends
}

pub struct Manager {
    commands: Sender<Command>,
    worker: Option<JoinHandle<()>>,
}

const REPLY_TIMEOUT: Duration = Duration::from_secs(2);

impl Manager {
    pub fn start(sink: SnapshotSink) -> std::io::Result<Self> {
        Self::with_backends(Box::new(default_backends), sink)
    }

    pub fn with_backends(factory: BackendFactory, sink: SnapshotSink) -> std::io::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let worker =
            std::thread::Builder::new().name("patchpad-input".into()).spawn(move || run(factory(), receiver, sink))?;
        Ok(Manager { commands, worker: Some(worker) })
    }

    fn request(&self, make: impl FnOnce(Sender<Result<(), OutputError>>) -> Command) -> Result<(), OutputError> {
        let (reply, response) = mpsc::channel();
        self.commands.send(make(reply)).map_err(|_| OutputError::Write("thread d'entrée arrêté".into()))?;
        response.recv_timeout(REPLY_TIMEOUT).map_err(|_| OutputError::Write("pas de réponse".into()))?
    }

    pub fn rumble(&self, id: &str, rumble: Rumble) -> Result<(), OutputError> {
        self.request(|reply| Command::Rumble { id: id.to_owned(), rumble, reply })
    }

    pub fn set_lightbar(&self, id: &str, rgb: [u8; 3]) -> Result<(), OutputError> {
        self.request(|reply| Command::Lightbar { id: id.to_owned(), rgb, reply })
    }

    pub fn reset_analysis(&self, id: Option<String>) {
        self.commands.send(Command::ResetAnalysis(id)).ok();
    }

    pub fn recalibrate_imu(&self, id: &str) {
        self.commands.send(Command::RecalibrateImu(id.to_owned())).ok();
    }

    pub fn set_drift_threshold(&self, threshold: f32) {
        self.commands.send(Command::SetDriftThreshold(threshold)).ok();
    }

    pub fn start_report(&self, id: &str) {
        self.commands.send(Command::StartReport(id.to_owned())).ok();
    }

    pub fn confirm_output(&self, id: &str, item: OutputItem, result: Confirmation) {
        self.commands.send(Command::ConfirmOutput { id: id.to_owned(), item, result }).ok();
    }

    /// Données du rapport de la manette `id`, ou `None` si elle n'est plus connectée.
    pub fn report_data(&self, id: &str) -> Option<ReportData> {
        let (reply, response) = mpsc::channel();
        self.commands.send(Command::ReportData { id: id.to_owned(), reply }).ok()?;
        response.recv_timeout(REPLY_TIMEOUT).ok().flatten()
    }

    pub fn shutdown(&mut self) {
        self.commands.send(Command::Shutdown).ok();
        if let Some(worker) = self.worker.take() {
            worker.join().ok();
        }
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(mut backends: Vec<Box<dyn ControllerBackend>>, commands: Receiver<Command>, mut sink: SnapshotSink) {
    let mut session = Session::new(Instant::now());
    let mut events = Vec::new();
    let mut last_emit: Option<Instant> = None;
    let mut dirty = true;
    let mut last_notices: Vec<Notice> = Vec::new();

    loop {
        let now = Instant::now();
        // Un backend ignore les VID/PID déjà ouverts par un backend prioritaire.
        let mut claimed: Vec<(u16, u16)> = Vec::new();
        for backend in &mut backends {
            backend.set_excluded_devices(&claimed);
            backend.poll(now, &mut events);
            claimed.extend(backend.claimed_devices());
        }
        dirty |= !events.is_empty();
        for event in events.drain(..) {
            session.apply(event, now);
        }

        let notices: Vec<Notice> = backends.iter().flat_map(|b| b.notices()).collect();
        if notices != last_notices {
            last_notices.clone_from(&notices);
            dirty = true;
        }

        let since_emit = last_emit.map(|t| now.saturating_duration_since(t));
        let due = since_emit.is_none_or(|d| (dirty && d >= EMIT_INTERVAL) || d >= IDLE_EMIT_INTERVAL);
        if due {
            sink(session.snapshot(now, notices));
            last_emit = Some(now);
            dirty = false;
        }

        match commands.recv_timeout(POLL_SLEEP) {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Ok(command) => {
                handle_command(command, &mut backends, &mut session, now);
                // Traiter d'un coup les autres commandes en attente.
                while let Ok(command) = commands.try_recv() {
                    if matches!(command, Command::Shutdown) {
                        return;
                    }
                    handle_command(command, &mut backends, &mut session, now);
                }
                dirty = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn handle_command(command: Command, backends: &mut [Box<dyn ControllerBackend>], session: &mut Session, now: Instant) {
    let owner = |backends: &mut [Box<dyn ControllerBackend>], id: &str| {
        let index = backends.iter().position(|b| b.owns(id));
        index.ok_or_else(|| OutputError::UnknownController(id.to_owned()))
    };
    match command {
        Command::Rumble { id, rumble, reply } => {
            let result = owner(backends, &id).and_then(|i| backends[i].rumble(&id, rumble, now));
            reply.send(result).ok();
        }
        Command::Lightbar { id, rgb, reply } => {
            let result = owner(backends, &id).and_then(|i| backends[i].set_lightbar(&id, rgb));
            reply.send(result).ok();
        }
        Command::ResetAnalysis(id) => session.reset_analysis(id.as_deref()),
        Command::RecalibrateImu(id) => {
            session.recalibrate_imu(&id);
        }
        Command::SetDriftThreshold(threshold) => session.set_drift_threshold(threshold),
        Command::StartReport(id) => {
            session.start_report(&id, now);
        }
        Command::ConfirmOutput { id, item, result } => {
            session.confirm_output(&id, item, result);
        }
        Command::ReportData { id, reply } => {
            reply.send(session.report_data(&id, now)).ok();
        }
        Command::Shutdown => {}
    }
}
