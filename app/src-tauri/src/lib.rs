//! Application Tauri : relaie les instantanés de `patchpad-core` au frontend et expose les
//! commandes de sortie (vibration, barre lumineuse) et d'analyse.

use std::sync::Mutex;
use std::time::Duration;

use patchpad_core::report::text::Lang;
use patchpad_core::report::{document, html, pdf, Confirmation, OutputItem};
use patchpad_core::{Manager, Rumble};
use serde::Deserialize;
use serde::Serialize;
use tauri::{Emitter, Manager as _, RunEvent, State};

/// Événement poussé au frontend, limité à ~60 par seconde côté Rust.
const SNAPSHOT_EVENT: &str = "pad://snapshot";
/// Durée maximale d'un test de vibration.
const MAX_RUMBLE_MS: u64 = 10_000;

struct AppState {
    manager: Mutex<Option<Manager>>,
}

impl AppState {
    fn with<T>(&self, f: impl FnOnce(&Manager) -> Result<T, String>) -> Result<T, String> {
        let guard = self.manager.lock().map_err(|_| "état verrouillé".to_owned())?;
        let manager = guard.as_ref().ok_or_else(|| "acquisition arrêtée".to_owned())?;
        f(manager)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformInfo {
    os: &'static str,
    /// Commande d'installation de la règle udev (Linux).
    udev_install: Option<String>,
}

/// Règle udev livrée dans `packaging/linux/` (installée automatiquement par le paquet .deb).
const UDEV_RULES: &str = include_str!("../../../packaging/linux/70-patchpad.rules");

#[tauri::command]
fn platform() -> PlatformInfo {
    let udev_install = cfg!(target_os = "linux").then(|| {
        let rules: Vec<&str> = UDEV_RULES.lines().filter(|l| l.starts_with("KERNEL")).collect();
        format!(
            "sudo tee /etc/udev/rules.d/70-patchpad.rules > /dev/null <<'EOF'\n{}\nEOF\n\
             sudo udevadm control --reload-rules && sudo udevadm trigger",
            rules.join("\n")
        )
    });
    PlatformInfo { os: std::env::consts::OS, udev_install }
}

#[tauri::command]
fn rumble(state: State<'_, AppState>, id: String, strong: f32, weak: f32, duration_ms: u64) -> Result<(), String> {
    let duration = Duration::from_millis(duration_ms.min(MAX_RUMBLE_MS));
    state.with(|m| m.rumble(&id, Rumble { strong, weak, duration }).map_err(|e| e.to_string()))
}

#[tauri::command]
fn set_lightbar(state: State<'_, AppState>, id: String, rgb: [u8; 3]) -> Result<(), String> {
    state.with(|m| m.set_lightbar(&id, rgb).map_err(|e| e.to_string()))
}

#[tauri::command]
fn reset_analysis(state: State<'_, AppState>, id: Option<String>) -> Result<(), String> {
    state.with(|m| {
        m.reset_analysis(id);
        Ok(())
    })
}

#[tauri::command]
fn recalibrate_imu(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.with(|m| {
        m.recalibrate_imu(&id);
        Ok(())
    })
}

#[tauri::command]
fn set_drift_threshold(state: State<'_, AppState>, threshold: f32) -> Result<(), String> {
    state.with(|m| {
        m.set_drift_threshold(threshold);
        Ok(())
    })
}

#[tauri::command]
fn start_report(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.with(|m| {
        m.start_report(&id);
        Ok(())
    })
}

#[tauri::command]
fn confirm_output(
    state: State<'_, AppState>,
    id: String,
    item: OutputItem,
    result: Confirmation,
) -> Result<(), String> {
    state.with(|m| {
        m.confirm_output(&id, item, result);
        Ok(())
    })
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum ReportFormat {
    Html,
    Pdf,
}

/// Génère le rapport de la manette `id` et l'écrit dans `path` (choisi par l'utilisateur).
/// `generated_at` : date et heure locales, formatées par le frontend.
#[tauri::command]
fn export_report(
    state: State<'_, AppState>,
    id: String,
    format: ReportFormat,
    path: String,
    lang: Lang,
    generated_at: String,
) -> Result<String, String> {
    let data = state.with(|m| m.report_data(&id).ok_or_else(|| "manette déconnectée".to_owned()))?;
    let doc = document::build(&data, lang, &generated_at);
    let bytes = match format {
        ReportFormat::Html => html::render(&doc).into_bytes(),
        ReportFormat::Pdf => pdf::render(&doc),
    };
    std::fs::write(&path, bytes).map_err(|e| format!("{path} : {e}"))?;
    Ok(path)
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let manager = Manager::start(Box::new(move |snapshot| {
                handle.emit(SNAPSHOT_EVENT, snapshot).ok();
            }))?;
            app.manage(AppState { manager: Mutex::new(Some(manager)) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            platform,
            rumble,
            set_lightbar,
            reset_analysis,
            recalibrate_imu,
            set_drift_threshold,
            start_report,
            confirm_output,
            export_report,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            eprintln!("PatchPad n'a pas pu démarrer : {error}");
            std::process::exit(1);
        }
    };

    app.run(|handle, event| {
        // Arrête le thread d'acquisition (et donc les vibrations en cours) avant de quitter.
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(mut manager) = state.manager.lock() {
                    manager.take();
                }
            }
        }
    });
}
