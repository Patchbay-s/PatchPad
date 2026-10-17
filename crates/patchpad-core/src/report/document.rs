//! Document de rapport indépendant du format : sections, lignes, verdicts et figures.

use super::text::{Lang, Text};
use super::{CheckStatus, Confirmation, ReportData, GYRO_MOVED_DPS};
use crate::analysis::stick::{DriftVerdict, StickReport};
use crate::analysis::trigger::TriggerReport;
use crate::model::{ReportMode, Support};

/// Rayon à partir duquel le stick est considéré comme atteignant le bord.
const EDGE_RADIUS: f32 = 0.95;
/// Couverture minimale pour juger « atteint le bord ».
const EDGE_MIN_COVERAGE: f32 = 0.75;
/// Une gâchette jamais enfoncée au-delà n'a pas été testée.
const TRIGGER_TESTED: f32 = 0.5;
/// Gravité au repos acceptable (g).
const GRAVITY_RANGE: (f32, f32) = (0.9, 1.1);

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub status: CheckStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StickFigure {
    pub title: String,
    /// Rayon max par secteur (secteur 0 = +X, sens trigonométrique).
    pub sectors: Vec<f32>,
    pub rest: Option<(f32, f32)>,
    pub threshold: f32,
    pub deadzone: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub title: String,
    pub rows: Vec<Row>,
    pub figures: Vec<StickFigure>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Summary {
    pub passed: usize,
    pub failed: usize,
    pub untested: usize,
    pub unavailable: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub lang: Lang,
    pub title: String,
    pub subtitle: String,
    pub generated: String,
    pub summary: Summary,
    pub summary_title: String,
    pub status_labels: StatusLabels,
    pub sections: Vec<Section>,
    pub footer: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StatusLabels {
    pub passed: String,
    pub failed: String,
    pub untested: String,
    pub unavailable: String,
}

impl StatusLabels {
    pub fn label(&self, status: CheckStatus) -> &str {
        match status {
            CheckStatus::Pass => &self.passed,
            CheckStatus::Fail => &self.failed,
            CheckStatus::Untested => &self.untested,
            CheckStatus::Unavailable => &self.unavailable,
            CheckStatus::Info => "",
        }
    }
}

fn row(label: impl Into<String>, value: impl Into<String>, status: CheckStatus) -> Row {
    Row { label: label.into(), value: value.into(), status }
}

/// `generated_at` : date et heure locales, formatées par l'appelant.
pub fn build(data: &ReportData, lang: Lang, generated_at: &str) -> Document {
    let t = Text::new(lang);
    let view = &data.view;
    let info = &view.info;
    let progress = &view.report;
    let mut sections = Vec::new();

    // --- Manette
    let mut rows = vec![
        row(t.name(), info.name.clone(), CheckStatus::Info),
        row(t.connection_label(), t.connection(info.connection), CheckStatus::Info),
        row(
            t.usb_id(),
            match (info.vendor_id, info.product_id) {
                (Some(v), Some(p)) => format!("{v:04x}:{p:04x}"),
                _ => "—".into(),
            },
            CheckStatus::Info,
        ),
        row(t.backend(), info.backend, CheckStatus::Info),
    ];
    if let Some(battery) = view.state.battery {
        let level = battery.level.map(|l| format!("{l} % · ")).unwrap_or_default();
        rows.push(row(t.battery(), format!("{level}{}", t.battery_status(battery.status)), CheckStatus::Info));
    }
    if let Some(mode) = info.report_mode {
        let (value, status) = match mode {
            ReportMode::Full => (t.full_mode(), CheckStatus::Info),
            ReportMode::Reduced => (t.reduced_mode(), CheckStatus::Fail),
        };
        rows.push(row(t.report_mode(), value, status));
    }
    let rate_label = if info.backend == "gilrs" { t.change_rate() } else { t.report_rate() };
    rows.push(row(
        rate_label,
        format!(
            "{} Hz · {} ms ± {} ms",
            t.num(view.rate.hz, 0),
            t.num(view.rate.mean_interval_ms, 2),
            t.num(view.rate.jitter_ms, 2)
        ),
        CheckStatus::Info,
    ));
    sections.push(Section { title: t.controller().into(), rows, figures: Vec::new() });

    // --- Boutons
    let rows = progress
        .buttons
        .iter()
        .map(|check| {
            let label = t.button(info.family, check.button);
            if !check.available {
                return row(label, t.unavailable_os(), CheckStatus::Unavailable);
            }
            match check.first_press_ms {
                Some(ms) => {
                    #[allow(clippy::cast_precision_loss)]
                    let seconds = ms as f32 / 1000.0;
                    row(
                        label,
                        format!("{} · {} {} s", t.presses(check.presses), t.first_press(), t.num(seconds, 1)),
                        CheckStatus::Pass,
                    )
                }
                None => row(label, "—", CheckStatus::Untested),
            }
        })
        .collect();
    sections.push(Section {
        title: format!("{} ({}/{})", t.buttons(), progress.buttons_tested, progress.buttons_total),
        rows,
        figures: Vec::new(),
    });

    // --- Sticks
    let mut rows = Vec::new();
    let mut figures = Vec::new();
    for (title, report) in [(t.left_stick(), &view.left_stick), (t.right_stick(), &view.right_stick)] {
        rows.extend(stick_rows(&t, title, report, data.drift_threshold));
        figures.push(StickFigure {
            title: title.into(),
            sectors: report.sectors.clone(),
            rest: report.rest.map(|s| (s.x, s.y)),
            threshold: data.drift_threshold,
            deadzone: report.suggested_deadzone,
        });
    }
    sections.push(Section { title: "Sticks".into(), rows, figures });

    // --- Gâchettes
    let mut rows = Vec::new();
    for (title, report) in [(t.left_trigger(), &view.left_trigger), (t.right_trigger(), &view.right_trigger)] {
        rows.extend(trigger_rows(&t, title, report.as_ref()));
    }
    sections.push(Section { title: t.triggers().into(), rows, figures: Vec::new() });

    // --- Pavé tactile
    if let Some(touch) = progress.touchpad {
        let seen = |b: bool| if b { (t.seen(), CheckStatus::Pass) } else { ("—", CheckStatus::Untested) };
        let (v1, s1) = seen(touch.finger_seen[0]);
        let (v2, s2) = seen(touch.finger_seen[1]);
        let (vb, sb) = seen(touch.both_at_once);
        // Le clic du pavé figure déjà dans la liste des boutons.
        let mut rows = vec![row(t.finger_1(), v1, s1), row(t.finger_2(), v2, s2), row(t.two_fingers(), vb, sb)];
        if let (Some((x0, x1)), Some((y0, y1)), Some(pad)) = (touch.x_range, touch.y_range, view.state.touchpad) {
            let pct = |a: u16, b: u16, total: u16| f32::from(b - a) / f32::from(total.max(1)) * 100.0;
            rows.push(row(
                t.covered_area(),
                format!(
                    "X {x0}–{x1} ({} %) · Y {y0}–{y1} ({} %)",
                    t.num(pct(x0, x1, pad.width - 1), 0),
                    t.num(pct(y0, y1, pad.height - 1), 0)
                ),
                CheckStatus::Info,
            ));
        }
        sections.push(Section { title: t.touchpad().into(), rows, figures: Vec::new() });
    }

    // --- Capteurs
    if let Some(imu) = progress.imu {
        let mut rows = Vec::new();
        let received = imu.samples > 0;
        rows.push(row(
            t.imu_data(),
            format!("{} {}", imu.samples, t.samples()),
            if received { CheckStatus::Pass } else { CheckStatus::Untested },
        ));
        let max = imu.max_gyro;
        let moved = max.iter().any(|m| *m >= GYRO_MOVED_DPS);
        rows.push(row(
            t.gyro_moved(),
            format!("max X {} · Y {} · Z {} °/s", t.num(max[0], 0), t.num(max[1], 0), t.num(max[2], 0)),
            if moved { CheckStatus::Pass } else { CheckStatus::Untested },
        ));
        rows.push(match imu.accel_rest_norm {
            Some(g) => row(
                t.gravity(),
                format!("{} g", t.num(g, 3)),
                if (GRAVITY_RANGE.0..=GRAVITY_RANGE.1).contains(&g) { CheckStatus::Pass } else { CheckStatus::Fail },
            ),
            None => row(t.gravity(), "—", CheckStatus::Untested),
        });
        if let Some(factory) = info.imu_factory_calibration {
            rows.push(row(t.calibration(), if factory { t.factory() } else { t.nominal() }, CheckStatus::Info));
        }
        sections.push(Section { title: t.motion().into(), rows, figures: Vec::new() });
    }

    // --- Sorties
    let outputs = &progress.outputs;
    let confirm = |support: Support, value: Confirmation| match (support, value) {
        (Support::Supported, Confirmation::Ok) => (t.confirmed(), CheckStatus::Pass),
        (Support::Supported, Confirmation::Failed) => (t.reported_ko(), CheckStatus::Fail),
        (Support::Supported, Confirmation::Untested) => ("—", CheckStatus::Untested),
        (Support::UnsupportedOs, _) => (t.unavailable_os(), CheckStatus::Unavailable),
        (Support::NoHardware, _) => (t.not_supported(), CheckStatus::Unavailable),
    };
    let mut rows = Vec::new();
    for (label, value) in [(t.rumble_left(), outputs.rumble_left), (t.rumble_right(), outputs.rumble_right)] {
        let (v, s) = confirm(info.capabilities.rumble, value);
        rows.push(row(label, v, s));
    }
    if info.capabilities.lightbar != Support::NoHardware {
        let (v, s) = confirm(info.capabilities.lightbar, outputs.lightbar);
        rows.push(row(t.lightbar(), v, s));
    }
    sections.push(Section { title: t.outputs().into(), rows, figures: Vec::new() });

    let mut summary = Summary::default();
    for r in sections.iter().flat_map(|s| &s.rows) {
        match r.status {
            CheckStatus::Pass => summary.passed += 1,
            CheckStatus::Fail => summary.failed += 1,
            CheckStatus::Untested => summary.untested += 1,
            CheckStatus::Unavailable => summary.unavailable += 1,
            CheckStatus::Info => {}
        }
    }

    Document {
        lang,
        title: t.title().into(),
        subtitle: info.name.clone(),
        generated: format!("{} {generated_at} · {} {} s", t.generated(), t.duration(), t.num(data.duration_s, 0)),
        summary,
        summary_title: t.summary().into(),
        status_labels: StatusLabels {
            passed: t.passed().into(),
            failed: t.failed().into(),
            untested: t.untested().into(),
            unavailable: t.unavailable().into(),
        },
        sections,
        footer: t.footer().into(),
    }
}

fn stick_rows(t: &Text, title: &str, report: &StickReport, threshold: f32) -> Vec<Row> {
    let mut rows = Vec::new();
    let label = |what: &str| format!("{title} · {what}");
    match report.rest {
        Some(rest) => {
            rows.push(row(
                label(t.rest_position()),
                format!("X {} · Y {}", t.num(rest.x, 3), t.num(rest.y, 3)),
                CheckStatus::Info,
            ));
            let status = match report.drift {
                DriftVerdict::Drift => CheckStatus::Fail,
                DriftVerdict::Ok => CheckStatus::Pass,
                DriftVerdict::Unknown => CheckStatus::Untested,
            };
            rows.push(row(
                label(t.drift()),
                format!(
                    "{} {} ({} {})",
                    t.rest_offset(),
                    t.num(report.rest_offset.unwrap_or(0.0), 3),
                    t.drift_threshold(),
                    t.num(threshold, 2)
                ),
                status,
            ));
        }
        None => rows.push(row(label(t.drift()), t.no_rest(), CheckStatus::Untested)),
    }
    if let Some(deadzone) = report.suggested_deadzone {
        rows.push(row(label(t.deadzone()), t.num(deadzone, 2), CheckStatus::Info));
    }
    let edge_status = if report.max_radius >= EDGE_RADIUS {
        CheckStatus::Pass
    } else if report.coverage >= EDGE_MIN_COVERAGE {
        CheckStatus::Fail
    } else {
        CheckStatus::Untested
    };
    rows.push(row(label(t.reach_edge()), format!("{} {}", t.max_radius(), t.num(report.max_radius, 3)), edge_status));
    rows.push(match report.circularity_error {
        Some(error) => row(
            label(t.circularity()),
            format!("{} % · {} {} %", t.num(error, 1), t.coverage(), t.num(report.coverage * 100.0, 0)),
            CheckStatus::Info,
        ),
        None => row(
            label(t.circularity()),
            format!("{} ({} {} %)", t.not_enough_coverage(), t.coverage(), t.num(report.coverage * 100.0, 0)),
            CheckStatus::Untested,
        ),
    });
    rows
}

fn trigger_rows(t: &Text, title: &str, report: Option<&TriggerReport>) -> Vec<Row> {
    let label = |what: &str| format!("{title} · {what}");
    let Some(report) = report else {
        return vec![row(label(t.travel()), "—", CheckStatus::Untested)];
    };
    let travel = format!("{} – {}", t.num(report.min, 3), t.num(report.max, 3));
    let full = if report.reaches_full {
        CheckStatus::Pass
    } else if report.max >= TRIGGER_TESTED {
        CheckStatus::Fail
    } else {
        CheckStatus::Untested
    };
    let zero = if report.reaches_zero { CheckStatus::Pass } else { CheckStatus::Fail };
    let yes_no = |b: bool| if b { t.yes() } else { t.no() };
    vec![
        row(label(t.travel()), travel, CheckStatus::Info),
        row(label(t.reaches_full()), yes_no(report.reaches_full), full),
        row(label(t.returns_zero()), yes_no(report.reaches_zero), zero),
    ]
}
