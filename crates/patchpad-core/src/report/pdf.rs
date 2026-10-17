//! Rendu PDF minimal, sans dépendance : PDF 1.4, polices standard Helvetica (encodage
//! WinAnsi, donc accents français pris en charge), logo JPEG intégré tel quel (DCTDecode),
//! A4, saut de page automatique et pied de page « page n/N ».

use std::fmt::Write as _;

use super::document::{Document, Row, StickFigure};
use super::html::outline_points;
use super::CheckStatus;

const MARK_JPEG: &[u8] = include_bytes!("../../assets/report-mark.jpg");
const MARK_PX: u32 = 160;

const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 50.0;
const BOTTOM: f32 = 60.0;
const CONTENT_W: f32 = PAGE_W - 2.0 * MARGIN;

const LABEL_X: f32 = MARGIN;
const LABEL_W: f32 = 200.0;
const VALUE_X: f32 = MARGIN + 210.0;
const VALUE_W: f32 = 200.0;
const BADGE_W: f32 = 76.0;
const ROW_SIZE: f32 = 9.5;
const LINE: f32 = 12.0;

type Rgb = (f32, f32, f32);
const FG: Rgb = (0.11, 0.17, 0.21);
const MUTED: Rgb = (0.42, 0.47, 0.50);
const BORDER: Rgb = (0.87, 0.89, 0.90);
const BRAND: Rgb = (0.12, 0.27, 0.38);
const ACCENT: Rgb = (0.88, 0.44, 0.16);
const PASS: Rgb = (0.12, 0.54, 0.23);
const FAIL: Rgb = (0.78, 0.16, 0.16);
const UNTESTED: Rgb = (0.54, 0.54, 0.56);
const UNAVAILABLE: Rgb = (0.70, 0.42, 0.0);

#[derive(Clone, Copy)]
enum Font {
    Regular,
    Bold,
}

struct Layout {
    pages: Vec<String>,
    current: String,
    /// Distance depuis le haut de la page.
    y: f32,
}

impl Layout {
    fn new() -> Self {
        Layout { pages: Vec::new(), current: String::new(), y: MARGIN }
    }

    fn ensure(&mut self, height: f32) {
        if self.y + height > PAGE_H - BOTTOM {
            self.new_page();
        }
    }

    fn new_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.current));
        self.y = MARGIN;
    }

    fn finish(mut self) -> Vec<String> {
        self.pages.push(self.current);
        self.pages
    }

    fn color(&mut self, (r, g, b): Rgb, stroke: bool) {
        let op = if stroke { "RG" } else { "rg" };
        let _ = writeln!(self.current, "{r:.3} {g:.3} {b:.3} {op}");
    }

    /// `top` : ligne de base mesurée depuis le haut de la page.
    fn text(&mut self, x: f32, top: f32, size: f32, font: Font, color: Rgb, text: &str) {
        self.color(color, false);
        let name = match font {
            Font::Regular => "F1",
            Font::Bold => "F2",
        };
        let _ = writeln!(
            self.current,
            "BT /{name} {size:.1} Tf {x:.2} {:.2} Td ({}) Tj ET",
            PAGE_H - top,
            pdf_string(text)
        );
    }

    fn line(&mut self, x1: f32, top1: f32, x2: f32, top2: f32, width: f32, color: Rgb) {
        self.color(color, true);
        let _ = writeln!(self.current, "{width:.2} w {x1:.2} {:.2} m {x2:.2} {:.2} l S", PAGE_H - top1, PAGE_H - top2);
    }

    fn rect(&mut self, x: f32, top: f32, w: f32, h: f32, color: Rgb, fill: bool) {
        self.color(color, !fill);
        let op = if fill { "f" } else { "S" };
        let _ = writeln!(self.current, "0.8 w {x:.2} {:.2} {w:.2} {h:.2} re {op}", PAGE_H - top - h);
    }

    /// Cercle approché par 4 courbes de Bézier.
    fn circle(&mut self, cx: f32, ctop: f32, r: f32, color: Rgb, fill: bool, dashed: bool) {
        const K: f32 = 0.552_284_8;
        let cy = PAGE_H - ctop;
        let k = K * r;
        self.color(color, !fill);
        if dashed {
            self.current.push_str("[3 3] 0 d\n");
        }
        let _ = write!(
            self.current,
            "0.8 w {:.2} {cy:.2} m {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {cy:.2} c \
             {:.2} {:.2} {:.2} {:.2} {cx:.2} {:.2} c {:.2} {:.2} {:.2} {:.2} {:.2} {cy:.2} c ",
            cx + r,
            cx + r,
            cy + k,
            cx + k,
            cy + r,
            cy + r,
            cx - k,
            cy + r,
            cx - r,
            cy + k,
            cx - r,
            cx - r,
            cy - k,
            cx - k,
            cy - r,
            cy - r,
            cx + k,
            cy - r,
            cx + r,
            cy - k,
            cx + r,
        );
        self.current.push_str(if fill { "f\n" } else { "S\n" });
        if dashed {
            self.current.push_str("[] 0 d\n");
        }
    }
}

pub fn render(doc: &Document) -> Vec<u8> {
    let mut l = Layout::new();

    // --- En-tête
    let logo = 56.0;
    let _ = writeln!(l.current, "q {logo:.2} 0 0 {logo:.2} {MARGIN:.2} {:.2} cm /Im1 Do Q", PAGE_H - MARGIN - logo);
    let tx = MARGIN + logo + 14.0;
    l.text(tx, MARGIN + 18.0, 18.0, Font::Bold, BRAND, &doc.title);
    l.text(tx, MARGIN + 36.0, 12.5, Font::Bold, FG, &doc.subtitle);
    l.text(tx, MARGIN + 51.0, 8.5, Font::Regular, MUTED, &doc.generated);
    l.y = MARGIN + logo + 10.0;
    l.line(MARGIN, l.y, PAGE_W - MARGIN, l.y, 2.0, BRAND);
    l.y += 26.0;

    // --- Résumé
    l.text(MARGIN, l.y, 12.0, Font::Bold, BRAND, &doc.summary_title);
    l.y += 10.0;
    let labels = &doc.status_labels;
    let s = &doc.summary;
    let tile_w = (CONTENT_W - 3.0 * 10.0) / 4.0;
    for (i, (label, count, color)) in [
        (&labels.passed, s.passed, PASS),
        (&labels.failed, s.failed, FAIL),
        (&labels.untested, s.untested, UNTESTED),
        (&labels.unavailable, s.unavailable, UNAVAILABLE),
    ]
    .into_iter()
    .enumerate()
    {
        #[allow(clippy::cast_precision_loss)]
        let x = MARGIN + i as f32 * (tile_w + 10.0);
        l.rect(x, l.y, tile_w, 46.0, BORDER, false);
        l.text(x + 10.0, l.y + 24.0, 18.0, Font::Bold, color, &count.to_string());
        l.text(x + 10.0, l.y + 38.0, 8.5, Font::Regular, MUTED, label);
    }
    l.y += 46.0;

    // --- Sections
    for section in &doc.sections {
        // Titre + au moins une ligne sur la même page.
        l.ensure(26.0 + 2.0 * LINE + 8.0);
        l.y += 26.0;
        l.text(MARGIN, l.y, 12.0, Font::Bold, BRAND, &section.title);
        l.y += 6.0;
        l.line(MARGIN, l.y, PAGE_W - MARGIN, l.y, 0.6, BORDER);
        for row in &section.rows {
            draw_row(&mut l, row, labels.label(row.status));
        }
        if !section.figures.is_empty() {
            draw_figures(&mut l, &section.figures);
        }
    }

    let pages = l.finish();
    let total = pages.len();
    let pages: Vec<String> = pages
        .into_iter()
        .enumerate()
        .map(|(i, mut content)| {
            let footer = format!("{} · {}/{}", doc.footer, i + 1, total);
            let width = text_width(&footer, 8.0, Font::Regular);
            let mut f = Layout::new();
            f.text((PAGE_W - width) / 2.0, PAGE_H - 30.0, 8.0, Font::Regular, MUTED, &footer);
            content.push_str(&f.current);
            content
        })
        .collect();
    assemble(&pages)
}

fn draw_row(l: &mut Layout, row: &Row, status_label: &str) {
    let label_lines = wrap(&row.label, LABEL_W, ROW_SIZE, Font::Regular);
    let value_lines = wrap(&row.value, VALUE_W, ROW_SIZE, Font::Regular);
    #[allow(clippy::cast_precision_loss)]
    let height = label_lines.len().max(value_lines.len()).max(1) as f32 * LINE + 8.0;
    l.ensure(height);
    let top = l.y;
    for (i, line) in label_lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let base = top + 4.0 + (i as f32 + 1.0) * LINE - 2.5;
        l.text(LABEL_X, base, ROW_SIZE, Font::Regular, FG, line);
    }
    for (i, line) in value_lines.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let base = top + 4.0 + (i as f32 + 1.0) * LINE - 2.5;
        l.text(VALUE_X, base, ROW_SIZE, Font::Regular, FG, line);
    }
    let color = match row.status {
        CheckStatus::Pass => Some(PASS),
        CheckStatus::Fail => Some(FAIL),
        CheckStatus::Untested => Some(UNTESTED),
        CheckStatus::Unavailable => Some(UNAVAILABLE),
        CheckStatus::Info => None,
    };
    if let Some(color) = color {
        let x = PAGE_W - MARGIN - BADGE_W;
        l.rect(x, top + 4.0, BADGE_W, 12.0, color, true);
        let width = text_width(status_label, 7.5, Font::Bold);
        l.text(x + (BADGE_W - width) / 2.0, top + 13.2, 7.5, Font::Bold, (1.0, 1.0, 1.0), status_label);
    }
    l.y += height;
    l.line(MARGIN, l.y, PAGE_W - MARGIN, l.y, 0.4, BORDER);
}

fn draw_figures(l: &mut Layout, figures: &[StickFigure]) {
    const R: f32 = 70.0;
    let height = 2.0 * R + 40.0;
    l.ensure(height);
    let top = l.y + 14.0;
    for (i, figure) in figures.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let cx = MARGIN + R + 10.0 + i as f32 * (2.0 * R + 50.0);
        let cy = top + R;
        l.circle(cx, cy, R, (0.965, 0.969, 0.976), true, false);
        l.circle(cx, cy, R, (0.79, 0.81, 0.83), false, false);
        l.line(cx - R, cy, cx + R, cy, 0.4, BORDER);
        l.line(cx, cy - R, cx, cy + R, 0.4, BORDER);
        l.circle(cx, cy, figure.threshold * R, MUTED, false, true);
        if let Some(deadzone) = figure.deadzone {
            l.circle(cx, cy, deadzone * R, UNAVAILABLE, false, false);
        }
        let points = outline_points(&figure.sectors);
        if let Some(((x0, y0), rest)) = points.split_first() {
            l.color(BRAND, true);
            let _ = write!(l.current, "1.5 w {:.2} {:.2} m ", cx + x0 * R, PAGE_H - (cy - y0 * R));
            for (x, y) in rest {
                let _ = write!(l.current, "{:.2} {:.2} l ", cx + x * R, PAGE_H - (cy - y * R));
            }
            l.current.push_str("S\n");
        }
        if let Some((x, y)) = figure.rest {
            l.circle(cx + x * R, cy - y * R, 3.0, ACCENT, true, false);
        }
        let width = text_width(&figure.title, 8.5, Font::Regular);
        l.text(cx - width / 2.0, cy + R + 14.0, 8.5, Font::Regular, MUTED, &figure.title);
    }
    l.y += height;
}

/// Écrit les objets PDF et la table xref.
fn assemble(pages: &[String]) -> Vec<u8> {
    // 1 catalogue, 2 pages, 3-4 polices, 5 image, puis (page, contenu) par page.
    let page_count = pages.len();
    let first_page_obj = 6;
    let kids: Vec<String> = (0..page_count).map(|i| format!("{} 0 R", first_page_obj + 2 * i)).collect();

    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        format!("<< /Type /Pages /Kids [{}] /Count {page_count} >>", kids.join(" ")).into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>".to_vec(),
    ];
    let mut image = format!(
        "<< /Type /XObject /Subtype /Image /Width {MARK_PX} /Height {MARK_PX} /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
        MARK_JPEG.len()
    )
    .into_bytes();
    image.extend_from_slice(MARK_JPEG);
    image.extend_from_slice(b"\nendstream");
    objects.push(image);

    for (i, content) in pages.iter().enumerate() {
        let content_obj = first_page_obj + 2 * i + 1;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> /XObject << /Im1 5 0 R >> >> \
                 /Contents {content_obj} 0 R >>"
            )
            .into_bytes(),
        );
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(content.as_bytes());
        stream.extend_from_slice(b"\nendstream");
        objects.push(stream);
    }

    // En-tête binaire recommandé pour signaler un fichier non textuel.
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for offset in offsets {
        let _ = writeln!(table, "{offset:010} 00000 n ");
    }
    let _ = write!(table, "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1);
    out.extend_from_slice(table.as_bytes());
    out
}

/// Chaîne littérale PDF en WinAnsi, avec échappement de `(`, `)` et `\`.
fn pdf_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.chars().map(winansi) {
        match byte {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(char::from(byte));
            }
            0x20..=0x7E => out.push(char::from(byte)),
            _ => {
                let _ = write!(out, "\\{byte:03o}");
            }
        }
    }
    out
}

/// Caractère Unicode → octet WinAnsi (cp1252). Inconnu → « ? ».
fn winansi(c: char) -> u8 {
    match c {
        ' '..='~' => u8::try_from(u32::from(c)).unwrap_or(b'?'),
        '\u{A0}' | '\u{202F}' | '\u{2009}' => b' ',
        '\u{A1}'..='\u{FF}' => u8::try_from(u32::from(c)).unwrap_or(b'?'),
        '€' => 0x80,
        '‚' => 0x82,
        '…' => 0x85,
        '‘' => 0x91,
        '’' => 0x92,
        '“' => 0x93,
        '”' => 0x94,
        '•' => 0x95,
        '–' => 0x96,
        '—' => 0x97,
        'Œ' => 0x8C,
        'œ' => 0x9C,
        _ => b'?',
    }
}

/// Largeurs Helvetica (AFM Adobe, 1/1000 em) pour les caractères 0x20..=0x7E.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ␠ ! " # $ % & ' ( ) * + , - . /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0-9 : ; < = > ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ A-O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // P-Z [ \ ] ^ _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` a-o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p-z { | } ~
];

fn text_width(text: &str, size: f32, font: Font) -> f32 {
    let units: u32 = text
        .chars()
        .map(|c| {
            let byte = winansi(c);
            match byte {
                0x20..=0x7E => u32::from(HELVETICA[usize::from(byte - 0x20)]),
                // Lettres accentuées et symboles : largeur moyenne d'une minuscule.
                _ => 556,
            }
        })
        .sum();
    #[allow(clippy::cast_precision_loss)]
    let width = units as f32 / 1000.0 * size;
    match font {
        Font::Regular => width,
        // Helvetica-Bold est environ 8 % plus large.
        Font::Bold => width * 1.08,
    }
}

/// Coupe le texte en lignes de largeur max `width` (mots entiers, mot trop long coupé).
fn wrap(text: &str, width: f32, size: f32, font: Font) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() { word.to_owned() } else { format!("{current} {word}") };
        if text_width(&candidate, size, font) <= width || current.is_empty() {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_owned();
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winansi_covers_french() {
        assert_eq!(pdf_string("Écart à 5 °"), "\\311cart \\340 5 \\260");
        assert_eq!(pdf_string("(a\\b)"), "\\(a\\\\b\\)");
        assert_eq!(pdf_string("–…✕"), "\\226\\205?");
    }

    #[test]
    fn widths_and_wrapping() {
        assert!((text_width("ii", 10.0, Font::Regular) - 4.44).abs() < 1e-3);
        let lines = wrap("un deux trois quatre cinq six sept", 60.0, 10.0, Font::Regular);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| text_width(l, 10.0, Font::Regular) <= 60.0));
        assert_eq!(wrap("", 50.0, 10.0, Font::Regular), vec![String::new()]);
    }
}
