//! Rendu HTML autonome (un seul fichier : styles, logo et figures intégrés).

use std::fmt::Write as _;

use super::document::{Document, StickFigure};
use super::text::Lang;
use super::CheckStatus;

const MARK_JPEG: &[u8] = include_bytes!("../../assets/report-mark.jpg");

const STYLE: &str = r#"
:root { --fg:#1c2b36; --muted:#6b7780; --border:#dfe3e6; --bg:#ffffff; --alt:#f6f7f9;
  --pass:#1f8a3b; --fail:#c62828; --untested:#8a8a8e; --unavailable:#b26b00; --brand:#1f4560; --accent:#e0702a; }
* { box-sizing: border-box; }
body { margin: 0; background: var(--alt); color: var(--fg);
  font: 14px/1.45 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
main { max-width: 880px; margin: 24px auto; background: var(--bg); padding: 32px 40px;
  border: 1px solid var(--border); border-radius: 12px; }
header { display: flex; align-items: center; gap: 18px; border-bottom: 3px solid var(--brand); padding-bottom: 16px; }
header img { width: 72px; height: 72px; }
h1 { margin: 0; font-size: 22px; color: var(--brand); }
.subtitle { font-size: 17px; font-weight: 600; }
.meta { color: var(--muted); font-size: 12px; }
.summary { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin: 20px 0 8px; }
.tile { border: 1px solid var(--border); border-radius: 10px; padding: 10px 12px; }
.tile b { display: block; font-size: 24px; }
.tile.pass b { color: var(--pass); } .tile.fail b { color: var(--fail); }
.tile.untested b { color: var(--untested); } .tile.unavailable b { color: var(--unavailable); }
h2 { font-size: 16px; color: var(--brand); margin: 26px 0 8px; }
table { width: 100%; border-collapse: collapse; }
td { padding: 6px 8px; border-bottom: 1px solid var(--border); vertical-align: top; }
td.label { width: 42%; }
td.status { width: 120px; text-align: right; white-space: nowrap; }
.badge { display: inline-block; padding: 1px 9px; border-radius: 999px; font-size: 12px; font-weight: 600; color: #fff; }
.badge.pass { background: var(--pass); } .badge.fail { background: var(--fail); }
.badge.untested { background: var(--untested); } .badge.unavailable { background: var(--unavailable); }
.figures { display: flex; gap: 24px; flex-wrap: wrap; margin-top: 12px; }
figure { margin: 0; text-align: center; font-size: 12px; color: var(--muted); }
footer { margin-top: 28px; color: var(--muted); font-size: 12px; text-align: center; }
@media print {
  body { background: #fff; }
  main { border: none; margin: 0; max-width: none; padding: 0; }
  h2 { break-after: avoid; } tr, figure { break-inside: avoid; }
  .badge { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
}
"#;

pub fn render(doc: &Document) -> String {
    let lang = match doc.lang {
        Lang::Fr => "fr",
        Lang::En => "en",
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{} · {}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n",
        esc(&doc.title),
        esc(&doc.subtitle)
    );
    let _ = writeln!(
        out,
        "<header><img alt=\"PatchPad\" src=\"data:image/jpeg;base64,{}\"><div><h1>{}</h1>\
         <div class=\"subtitle\">{}</div><div class=\"meta\">{}</div></div></header>",
        base64(MARK_JPEG),
        esc(&doc.title),
        esc(&doc.subtitle),
        esc(&doc.generated)
    );

    let labels = &doc.status_labels;
    let s = &doc.summary;
    let _ = write!(out, "<h2>{}</h2>\n<div class=\"summary\">", esc(&doc.summary_title));
    for (class, label, count) in [
        ("pass", &labels.passed, s.passed),
        ("fail", &labels.failed, s.failed),
        ("untested", &labels.untested, s.untested),
        ("unavailable", &labels.unavailable, s.unavailable),
    ] {
        let _ = write!(out, "<div class=\"tile {class}\"><b>{count}</b>{}</div>", esc(label));
    }
    out.push_str("</div>\n");

    for section in &doc.sections {
        let _ = write!(out, "<section>\n<h2>{}</h2>\n<table>\n", esc(&section.title));
        for row in &section.rows {
            let badge = match status_class(row.status) {
                Some(class) => {
                    format!("<span class=\"badge {class}\">{}</span>", esc(labels.label(row.status)))
                }
                None => String::new(),
            };
            let _ = writeln!(
                out,
                "<tr><td class=\"label\">{}</td><td>{}</td><td class=\"status\">{badge}</td></tr>",
                esc(&row.label),
                esc(&row.value)
            );
        }
        out.push_str("</table>\n");
        if !section.figures.is_empty() {
            out.push_str("<div class=\"figures\">");
            for figure in &section.figures {
                out.push_str(&stick_svg(figure));
            }
            out.push_str("</div>\n");
        }
        out.push_str("</section>\n");
    }
    let _ = write!(out, "<footer>{}</footer>\n</main>\n</body>\n</html>\n", esc(&doc.footer));
    out
}

fn status_class(status: CheckStatus) -> Option<&'static str> {
    match status {
        CheckStatus::Pass => Some("pass"),
        CheckStatus::Fail => Some("fail"),
        CheckStatus::Untested => Some("untested"),
        CheckStatus::Unavailable => Some("unavailable"),
        CheckStatus::Info => None,
    }
}

/// Cercle unité, seuil de drift, zone morte conseillée, contour mesuré et position de repos.
fn stick_svg(figure: &StickFigure) -> String {
    const R: f32 = 90.0;
    let mut svg = String::from(
        "<figure><svg width=\"220\" height=\"220\" viewBox=\"-110 -110 220 220\" role=\"img\">\
         <circle r=\"90\" fill=\"#f6f7f9\" stroke=\"#c9ced3\"/>\
         <path d=\"M-90 0H90M0 -90V90\" stroke=\"#dfe3e6\"/>",
    );
    let _ = write!(
        svg,
        "<circle r=\"{:.1}\" fill=\"none\" stroke=\"#6b7780\" stroke-dasharray=\"3 3\"/>",
        figure.threshold * R
    );
    if let Some(deadzone) = figure.deadzone {
        let _ = write!(svg, "<circle r=\"{:.1}\" fill=\"none\" stroke=\"#b26b00\"/>", deadzone * R);
    }
    let points = outline_points(&figure.sectors);
    if !points.is_empty() {
        let list: Vec<String> = points.iter().map(|(x, y)| format!("{:.1},{:.1}", x * R, -y * R)).collect();
        let _ = write!(
            svg,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#1f4560\" stroke-width=\"2\"/>",
            list.join(" ")
        );
    }
    if let Some((x, y)) = figure.rest {
        let _ = write!(svg, "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"4\" fill=\"#e0702a\"/>", x * R, -y * R);
    }
    let _ = write!(svg, "</svg><figcaption>{}</figcaption></figure>", esc(&figure.title));
    svg
}

/// Points du contour mesuré (secteurs visités), fermé si tous les secteurs le sont.
pub(crate) fn outline_points(sectors: &[f32]) -> Vec<(f32, f32)> {
    let n = sectors.len();
    if n == 0 || sectors.iter().all(|r| *r <= 0.0) {
        return Vec::new();
    }
    #[allow(clippy::cast_precision_loss)]
    let step = std::f32::consts::TAU / n as f32;
    let mut points: Vec<(f32, f32)> = sectors
        .iter()
        .enumerate()
        .filter(|(_, r)| **r > 0.0)
        .map(|(i, r)| {
            #[allow(clippy::cast_precision_loss)]
            let angle = (i as f32 + 0.5) * step;
            (angle.cos() * r, angle.sin() * r)
        })
        .collect();
    if sectors.iter().all(|r| *r > 0.0) {
        if let Some(first) = points.first().copied() {
            points.push(first);
        }
    }
    points
}

pub(crate) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(TABLE[((n >> shift) & 0x3F) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(esc("<a href=\"x\">&'"), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;");
    }

    #[test]
    fn outline_closes_only_when_complete() {
        assert!(outline_points(&[0.0; 8]).is_empty());
        assert_eq!(outline_points(&[1.0, 0.0, 1.0, 0.0]).len(), 2);
        assert_eq!(outline_points(&[1.0; 4]).len(), 5);
    }
}
