//! Rapport : suivi d'une session simulée, puis rendu HTML et PDF.

// Fichier de test : les helpers hors #[test] peuvent aussi échouer bruyamment.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::time::{Duration, Instant};

use patchpad_core::backend::BackendEvent;
use patchpad_core::manager::Session;
use patchpad_core::model::{
    Button, Capabilities, Connection, ControllerInfo, ControllerState, Family, Finger, Imu, ReportMode, Stick, Support,
    Touchpad,
};
use patchpad_core::report::document::{build, Document};
use patchpad_core::report::text::Lang;
use patchpad_core::report::{html, pdf, CheckStatus, Confirmation, OutputItem, ReportData};

fn ds4_info() -> ControllerInfo {
    ControllerInfo {
        id: "ds4:test".into(),
        name: "DualShock 4 (v2)".into(),
        family: Family::DualShock4,
        backend: "hidapi",
        connection: Connection::Usb,
        vendor_id: Some(0x054C),
        product_id: Some(0x09CC),
        capabilities: Capabilities {
            touchpad: true,
            imu: true,
            rumble: Support::Supported,
            lightbar: Support::Supported,
            analog_triggers: true,
        },
        report_mode: Some(ReportMode::Full),
        imu_factory_calibration: Some(true),
        dongle_idle: false,
    }
}

/// Session : 3 boutons appuyés, stick gauche décalé (drift), tour complet du stick droit,
/// gâchette gauche sur toute sa course, deux doigts sur le pavé, manette secouée puis posée.
fn session_data() -> ReportData {
    let start = Instant::now();
    let mut session = Session::new(start);
    session.apply(BackendEvent::Connected(ds4_info()), start);
    let mut t = start;
    let mut push = |session: &mut Session, state: ControllerState| {
        t += Duration::from_millis(4);
        session.apply(BackendEvent::Input { id: "ds4:test".into(), state, at: t }, t);
    };
    let base = ControllerState {
        left_stick: Stick { x: 0.08, y: 0.0 },
        imu: Some(Imu { gyro: [0.0; 3], accel: [0.0, 1.0, 0.0] }),
        ..Default::default()
    };
    for button in [Button::South, Button::L1, Button::DpadUp] {
        for pressed in [true, false] {
            let buttons = if pressed { [button].into_iter().collect() } else { Default::default() };
            for _ in 0..20 {
                push(&mut session, ControllerState { buttons, ..base });
            }
        }
    }
    for _ in 0..200 {
        push(&mut session, base);
    }
    for i in 0..720u32 {
        let angle = (i as f32 / 2.0).to_radians();
        let trigger = (i as f32 / 360.0).min(1.0);
        push(
            &mut session,
            ControllerState {
                right_stick: Stick { x: angle.cos(), y: angle.sin() },
                left_trigger: if i == 719 { 0.0 } else { trigger },
                ..base
            },
        );
    }
    let finger = |x, y| Finger { active: true, id: 3, x, y };
    push(
        &mut session,
        ControllerState {
            touchpad: Some(Touchpad {
                fingers: [finger(10, 20), finger(1900, 930)],
                clicked: true,
                width: 1920,
                height: 942,
            }),
            buttons: [Button::Touchpad].into_iter().collect(),
            imu: Some(Imu { gyro: [0.0, 200.0, 0.0], accel: [0.0, 1.0, 0.0] }),
            ..base
        },
    );
    session.confirm_output("ds4:test", OutputItem::RumbleLeft, Confirmation::Ok);
    session.confirm_output("ds4:test", OutputItem::Lightbar, Confirmation::Failed);
    session.report_data("ds4:test", t).unwrap()
}

fn find<'a>(doc: &'a Document, label: &str) -> &'a patchpad_core::report::document::Row {
    doc.sections.iter().flat_map(|s| &s.rows).find(|r| r.label == label).unwrap_or_else(|| panic!("{label}"))
}

#[test]
fn document_verdicts() {
    let data = session_data();
    assert_eq!(data.view.report.buttons_tested, 4);
    assert_eq!(data.view.report.buttons_total, 18);
    let doc = build(&data, Lang::Fr, "17/10/2026 21:00");

    assert_eq!(find(&doc, "Croix").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Rond").status, CheckStatus::Untested);
    assert_eq!(find(&doc, "Croix").value, "1 appui · premier à 0,0 s");
    assert_eq!(find(&doc, "Stick gauche · Drift").status, CheckStatus::Fail);
    assert_eq!(find(&doc, "Stick droit · Atteint le bord").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Stick gauche · Atteint le bord").status, CheckStatus::Untested);
    assert_eq!(find(&doc, "Gâchette gauche · atteint 100 %").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Gâchette gauche · revient à 0").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Gâchette droite · atteint 100 %").status, CheckStatus::Untested);
    assert_eq!(find(&doc, "Deux doigts simultanés").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Clic du pavé").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Gyroscope (mouvement détecté)").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Gravité au repos").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Vibration gauche (fort)").status, CheckStatus::Pass);
    assert_eq!(find(&doc, "Vibration droite (faible)").status, CheckStatus::Untested);
    assert_eq!(find(&doc, "Barre lumineuse").status, CheckStatus::Fail);

    let s = doc.summary;
    let counted = doc.sections.iter().flat_map(|s| &s.rows).filter(|r| r.status != CheckStatus::Info).count();
    assert_eq!(s.passed + s.failed + s.untested + s.unavailable, counted);
    assert!(s.failed >= 2);

    let en = build(&data, Lang::En, "10/17/2026 9:00 PM");
    assert_eq!(find(&en, "Cross").status, CheckStatus::Pass);
    assert_eq!(en.title, "Controller test report");
}

#[test]
fn html_is_self_contained() {
    let doc = build(&session_data(), Lang::Fr, "17/10/2026 21:00");
    let out = html::render(&doc);
    assert!(out.starts_with("<!doctype html>"));
    assert!(out.contains("<html lang=\"fr\">"));
    assert!(out.contains("data:image/jpeg;base64,/9j/"), "logo JPEG intégré");
    assert!(out.contains("Rapport de test de manette"));
    assert!(out.contains("Boutons (4/18)"));
    assert_eq!(out.matches("<svg").count(), 2, "un schéma par stick");
    assert!(!out.contains("src=\"http"), "aucune ressource externe");
}

#[test]
fn pdf_structure_is_valid() {
    let doc = build(&session_data(), Lang::Fr, "17/10/2026 21:00");
    let bytes = pdf::render(&doc);
    assert!(bytes.starts_with(b"%PDF-1.4\n"));
    assert!(bytes.ends_with(b"%%EOF\n"));

    // startxref pointe sur la table xref.
    let tail = std::str::from_utf8(&bytes[find_last(&bytes, b"\nxref\n").unwrap() + 1..]).unwrap();
    let startxref: usize = tail.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
    assert!(bytes[startxref..].starts_with(b"xref\n"));
    // Chaque entrée de la table pointe sur « N 0 obj ».
    let entries: Vec<usize> =
        tail.lines().skip(3).take_while(|l| l.ends_with(" n ")).map(|l| l[..10].parse().unwrap()).collect();
    assert!(entries.len() >= 7, "{entries:?}");
    for (i, offset) in entries.iter().enumerate() {
        let header = format!("{} 0 obj", i + 1);
        assert!(bytes[*offset..].starts_with(header.as_bytes()), "objet {} mal référencé", i + 1);
    }
    // Les longueurs des flux sont exactes : « /Length n >>\nstream\n » + n octets + « \nendstream ».
    let mut streams = 0;
    let mut from = 0;
    while let Some(pos) = find_from(&bytes, b"/Length ", from) {
        let digits: Vec<u8> = bytes[pos + 8..].iter().copied().take_while(u8::is_ascii_digit).collect();
        let length: usize = std::str::from_utf8(&digits).unwrap().parse().unwrap();
        let body = find_from(&bytes, b"stream\n", pos).unwrap() + b"stream\n".len();
        assert!(bytes[body + length..].starts_with(b"\nendstream"), "flux à l'octet {pos}");
        streams += 1;
        from = body + length;
    }
    // 5 objets fixes puis (page, contenu) par page : une image + un flux de contenu par page.
    assert_eq!(streams, 1 + (entries.len() - 5) / 2);
    let text = String::from_utf8_lossy(&bytes);
    // Texte en WinAnsi : « Gâchettes » → G\342chettes.
    assert!(text.contains("(G\\342chettes)"));
    assert!(text.contains("/DCTDecode"));
}

#[test]
fn long_reports_paginate() {
    let mut doc = build(&session_data(), Lang::En, "now");
    let rows = doc.sections[1].rows.clone();
    for _ in 0..6 {
        doc.sections[1].rows.extend(rows.clone());
    }
    let bytes = pdf::render(&doc);
    let text = String::from_utf8_lossy(&bytes);
    let pages = text.matches("/Type /Page ").count();
    assert!(pages >= 3, "{pages} pages");
    assert!(text.contains(&format!("/Count {pages}")));
    assert!(text.contains(&format!("1/{pages})")));
}

fn find_from(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

fn find_last(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).rposition(|w| w == needle)
}

#[test]
#[ignore]
fn write_samples() {
    let data = session_data();
    let dir = std::env::var("PATCHPAD_SAMPLES").unwrap();
    for (lang, suffix) in [(Lang::Fr, "fr"), (Lang::En, "en")] {
        let doc = build(&data, lang, "17/10/2026 21:00");
        std::fs::write(format!("{dir}/rapport-{suffix}.html"), html::render(&doc)).unwrap();
        std::fs::write(format!("{dir}/rapport-{suffix}.pdf"), pdf::render(&doc)).unwrap();
    }
}
