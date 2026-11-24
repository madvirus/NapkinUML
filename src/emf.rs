//! Pictures in Windows' Enhanced Metafile format (EMF), made from egui's shapes.
//!
//! An EMF holds drawing commands (rectangles, ellipses, lines, polygons, text) instead of
//! pixels. PowerPoint, Word and other Office programs paste it as a picture that can be ungrouped
//! (Ctrl+Shift+G) into their own shapes and text boxes, which can then be edited one by one.
//!
//! The diagram is painted with the same drawing code as the canvas; each shape egui would draw
//! becomes one EMF record: rectangles and rounded rectangles, ellipses, polygons, lines (curves
//! become short lines) and text (one record per line of text, with each letter's place, in the
//! font it was drawn with). Pictures inside the diagram (triangle meshes) are left out.

use egui::epaint::{self, Color32, Shape, Stroke};
use egui::{Pos2, Rect, Vec2};

/// Logical EMF units per diagram unit: 4, so a diagram unit (a pixel at 100%) is 1/96 inch
/// with a quarter-unit precision (the picture's device is 384 dots per inch).
pub const K: f32 = 4.0;
const DPI: f32 = 96.0 * K;

// record types (MS-EMF 2.1.1)
const EMR_HEADER: u32 = 1;
const EMR_POLYGON: u32 = 3;
const EMR_POLYLINE: u32 = 4;
const EMR_EOF: u32 = 14;
const EMR_SETBKMODE: u32 = 18;
const EMR_SETTEXTALIGN: u32 = 22;
const EMR_SETTEXTCOLOR: u32 = 24;
const EMR_SELECTOBJECT: u32 = 37;
const EMR_CREATEPEN: u32 = 38;
const EMR_CREATEBRUSHINDIRECT: u32 = 39;
const EMR_DELETEOBJECT: u32 = 40;
const EMR_ELLIPSE: u32 = 42;
const EMR_RECTANGLE: u32 = 43;
const EMR_ROUNDRECT: u32 = 44;
const EMR_EXTCREATEFONTINDIRECTW: u32 = 82;
const EMR_EXTTEXTOUTW: u32 = 84;
const EMR_EXTCREATEPEN: u32 = 95;

// object handles: one pen, one brush and one font at a time
const H_PEN: u32 = 1;
const H_BRUSH: u32 = 2;
const H_FONT: u32 = 3;
// stock objects, selected before a handle is deleted
const STOCK_WHITE_BRUSH: u32 = 0x8000_0000;
const STOCK_BLACK_PEN: u32 = 0x8000_0007;
const STOCK_SYSTEM_FONT: u32 = 0x8000_000D;

/// An EMF being written: its records after the header, which `finish` puts in front.
pub struct Emf {
    body: Vec<u8>,
    records: u32,
    bounds: Option<(i32, i32, i32, i32)>,
    pen: Option<Option<(u32, i32, bool)>>,
    brush: Option<Option<u32>>,
    font: Option<(String, i32, i32)>,
    text_color: Option<u32>,
    /// Fonts for egui's own families (the built-in font isn't on Windows): Latin text, text
    /// with Korean, Chinese or Japanese letters, and fixed-width text.
    pub proportional_face: String,
    pub cjk_face: String,
    pub monospace_face: String,
}

fn le(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn lei(v: &mut Vec<u8>, x: i32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// A colour as a Windows COLORREF, drawn over white when partly see-through; None when it is
/// (nearly) invisible.
fn colorref(c: Color32) -> Option<u32> {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a < 8 {
        return None;
    }
    let k = a as f32 / 255.0;
    let mix = |v: u8| (v as f32 * k + 255.0 * (1.0 - k)).round().clamp(0.0, 255.0) as u32;
    Some(mix(r) | (mix(g) << 8) | (mix(b) << 16))
}

impl Default for Emf {
    fn default() -> Self {
        Self::new()
    }
}

impl Emf {
    pub fn new() -> Self {
        Emf {
            body: Vec::new(),
            records: 0,
            bounds: None,
            pen: None,
            brush: None,
            font: None,
            text_color: None,
            proportional_face: "Segoe UI".into(),
            cjk_face: "Malgun Gothic".into(),
            monospace_face: "Consolas".into(),
        }
    }

    fn rec(&mut self, typ: u32, data: &[u8]) {
        let mut d = data.to_vec();
        while d.len() % 4 != 0 {
            d.push(0);
        }
        le(&mut self.body, typ);
        le(&mut self.body, 8 + d.len() as u32);
        self.body.extend_from_slice(&d);
        self.records += 1;
    }

    fn rec_u32(&mut self, typ: u32, x: u32) {
        self.rec(typ, &x.to_le_bytes());
    }

    fn pt(&mut self, p: Pos2) -> (i32, i32) {
        let q = ((p.x * K).round() as i32, (p.y * K).round() as i32);
        self.bounds = Some(match self.bounds {
            None => (q.0, q.1, q.0, q.1),
            Some((a, b, c, d)) => (a.min(q.0), b.min(q.1), c.max(q.0), d.max(q.1)),
        });
        q
    }

    /// The pen for outlines and lines: a colour and width, or none.
    fn set_pen(&mut self, stroke: Option<(Color32, f32)>) {
        self.set_pen_dashed(stroke, false);
    }

    /// The pen, solid or dashed.
    fn set_pen_dashed(&mut self, stroke: Option<(Color32, f32)>, dashed: bool) {
        let key = stroke.and_then(|(c, w)| colorref(c).filter(|_| w > 0.0).map(|c| (c, ((w * K).round() as i32).max(1), dashed)));
        if self.pen == Some(key) {
            return;
        }
        if self.pen.is_some() {
            self.rec_u32(EMR_SELECTOBJECT, STOCK_BLACK_PEN);
            self.rec_u32(EMR_DELETEOBJECT, H_PEN);
        }
        let mut d = vec![];
        le(&mut d, H_PEN);
        match key {
            Some((c, w, true)) => {
                // a geometric dashed pen (a plain pen wider than one unit can't be dashed)
                for _ in 0..4 {
                    le(&mut d, 0); // no brush bitmap
                }
                le(&mut d, 0x0001_0000 | 0x200 | 1); // PS_GEOMETRIC | PS_ENDCAP_FLAT | PS_DASH
                le(&mut d, w as u32);
                le(&mut d, 0); // BS_SOLID
                le(&mut d, c);
                le(&mut d, 0);
                le(&mut d, 0); // no style entries
                self.rec(EMR_EXTCREATEPEN, &d);
                self.rec_u32(EMR_SELECTOBJECT, H_PEN);
                self.pen = Some(key);
                return;
            }
            Some((c, w, false)) => {
                le(&mut d, 0); // PS_SOLID
                lei(&mut d, w);
                lei(&mut d, 0);
                le(&mut d, c);
            }
            None => {
                le(&mut d, 5); // PS_NULL
                lei(&mut d, 0);
                lei(&mut d, 0);
                le(&mut d, 0);
            }
        }
        self.rec(EMR_CREATEPEN, &d);
        self.rec_u32(EMR_SELECTOBJECT, H_PEN);
        self.pen = Some(key);
    }

    /// The brush that fills shapes: a colour, or none.
    fn set_brush(&mut self, fill: Option<Color32>) {
        let key = fill.and_then(colorref);
        if self.brush == Some(key) {
            return;
        }
        if self.brush.is_some() {
            self.rec_u32(EMR_SELECTOBJECT, STOCK_WHITE_BRUSH);
            self.rec_u32(EMR_DELETEOBJECT, H_BRUSH);
        }
        let mut d = vec![];
        le(&mut d, H_BRUSH);
        le(&mut d, if key.is_some() { 0 } else { 1 }); // BS_SOLID / BS_NULL
        le(&mut d, key.unwrap_or(0));
        le(&mut d, 0);
        self.rec(EMR_CREATEBRUSHINDIRECT, &d);
        self.rec_u32(EMR_SELECTOBJECT, H_BRUSH);
        self.brush = Some(key);
    }

    fn set_font(&mut self, face: &str, height: i32, escapement: i32) {
        let key = (face.to_string(), height, escapement);
        if self.font.as_ref() == Some(&key) {
            return;
        }
        if self.font.is_some() {
            self.rec_u32(EMR_SELECTOBJECT, STOCK_SYSTEM_FONT);
            self.rec_u32(EMR_DELETEOBJECT, H_FONT);
        }
        let mut d = vec![];
        le(&mut d, H_FONT);
        // LOGFONTW
        lei(&mut d, -height.max(1)); // a negative height: the letters' em size
        lei(&mut d, 0);
        lei(&mut d, escapement);
        lei(&mut d, escapement);
        lei(&mut d, 400); // FW_NORMAL
        d.extend_from_slice(&[0, 0, 0, 1]); // not italic, underlined or struck out; DEFAULT_CHARSET
        d.extend_from_slice(&[0, 0, 5, 0]); // default precision, CLEARTYPE_QUALITY, default pitch
        let mut name: Vec<u16> = face.encode_utf16().take(31).collect();
        name.resize(32, 0);
        for u in name {
            d.extend_from_slice(&u.to_le_bytes());
        }
        self.rec(EMR_EXTCREATEFONTINDIRECTW, &d);
        self.rec_u32(EMR_SELECTOBJECT, H_FONT);
        self.font = Some(key);
    }

    fn rect_rec(&mut self, typ: u32, r: Rect, corner: Option<f32>) {
        let a = self.pt(r.min);
        let b = self.pt(r.max);
        let mut d = vec![];
        for v in [a.0, a.1, b.0, b.1] {
            lei(&mut d, v);
        }
        if let Some(c) = corner {
            let c = (c * 2.0 * K).round() as i32;
            lei(&mut d, c);
            lei(&mut d, c);
        }
        self.rec(typ, &d);
    }

    fn poly(&mut self, typ: u32, pts: &[Pos2]) {
        if pts.len() < 2 {
            return;
        }
        let q: Vec<(i32, i32)> = pts.iter().map(|p| self.pt(*p)).collect();
        let mut d = vec![];
        let (x0, y0) = (q.iter().map(|p| p.0).min().unwrap(), q.iter().map(|p| p.1).min().unwrap());
        let (x1, y1) = (q.iter().map(|p| p.0).max().unwrap(), q.iter().map(|p| p.1).max().unwrap());
        for v in [x0, y0, x1, y1] {
            lei(&mut d, v);
        }
        le(&mut d, q.len() as u32);
        for (x, y) in q {
            lei(&mut d, x);
            lei(&mut d, y);
        }
        self.rec(typ, &d);
    }

    fn stroke_of(s: Stroke) -> Option<(Color32, f32)> {
        (s.width > 0.0 && s.color.a() > 0).then_some((s.color, s.width))
    }

    fn path_stroke(s: &epaint::PathStroke) -> Option<(Color32, f32)> {
        match s.color {
            epaint::ColorMode::Solid(c) if s.width > 0.0 && c.a() > 0 => Some((c, s.width)),
            _ => None,
        }
    }

    /// A path: filled and outlined when closed, a line otherwise.
    fn path(&mut self, pts: &[Pos2], closed: bool, fill: Color32, stroke: Option<(Color32, f32)>) {
        let filled = closed && colorref(fill).is_some();
        if pts.len() < 2 || (!filled && stroke.is_none()) {
            return;
        }
        if closed {
            self.set_pen(stroke);
            self.set_brush(filled.then_some(fill));
            self.poly(EMR_POLYGON, pts);
        } else {
            self.set_pen(stroke);
            self.poly(EMR_POLYLINE, pts);
        }
    }

    /// Add egui's shapes in order. The pieces egui draws a dashed line with (one short line each)
    /// are joined back into one dashed line, so it stays one line when ungrouped in PowerPoint.
    pub fn add_shapes<'a>(&mut self, shapes: impl IntoIterator<Item = &'a Shape>) {
        fn flatten<'a>(s: &'a Shape, out: &mut Vec<&'a Shape>) {
            match s {
                Shape::Vec(v) => v.iter().for_each(|x| flatten(x, out)),
                _ => out.push(s),
            }
        }
        let mut flat = vec![];
        for s in shapes {
            flatten(s, &mut flat);
        }
        let mut i = 0;
        while i < flat.len() {
            if let Shape::LineSegment { points, stroke } = flat[i] {
                let n = dash_run(&flat[i..], *points, *stroke);
                if n >= 3 {
                    let mut pts = vec![points[0], points[1]];
                    for s in &flat[i + 1..i + n] {
                        if let Shape::LineSegment { points: q, .. } = s {
                            pts.push(q[0]);
                            pts.push(q[1]);
                        }
                    }
                    if let Some(s) = Self::stroke_of(*stroke) {
                        self.set_pen_dashed(Some(s), true);
                        self.poly(EMR_POLYLINE, &simplify(&pts));
                    }
                    i += n;
                    continue;
                }
            }
            self.add_shape(flat[i]);
            i += 1;
        }
    }

    /// Add one of egui's shapes (and everything in a group of them).
    pub fn add_shape(&mut self, shape: &Shape) {
        match shape {
            Shape::Vec(v) => {
                for s in v {
                    self.add_shape(s);
                }
            }
            Shape::Rect(r) => {
                // soft shadows have no outline to keep
                if r.blur_width > 0.0 || r.rect.width() <= 0.0 || r.rect.height() <= 0.0 {
                    return;
                }
                let stroke = Self::stroke_of(r.stroke);
                if colorref(r.fill).is_none() && stroke.is_none() {
                    return;
                }
                self.set_pen(stroke);
                self.set_brush(Some(r.fill));
                let cr = r.corner_radius;
                let rad = (cr.nw.max(cr.ne).max(cr.sw).max(cr.se) as f32).min(r.rect.width() / 2.0).min(r.rect.height() / 2.0);
                if rad < 0.5 {
                    self.rect_rec(EMR_RECTANGLE, r.rect, None);
                } else {
                    self.rect_rec(EMR_ROUNDRECT, r.rect, Some(rad));
                }
            }
            Shape::Circle(c) => self.ellipse(c.center, Vec2::splat(c.radius), c.fill, c.stroke),
            Shape::Ellipse(e) => self.ellipse(e.center, e.radius, e.fill, e.stroke),
            Shape::LineSegment { points, stroke } => {
                if let Some(s) = Self::stroke_of(*stroke) {
                    self.set_pen(Some(s));
                    self.poly(EMR_POLYLINE, points);
                }
            }
            Shape::Path(p) => self.path(&p.points, p.closed, p.fill, Self::path_stroke(&p.stroke)),
            Shape::CubicBezier(b) => self.path(&b.flatten(Some(0.1)), b.closed, b.fill, Self::path_stroke(&b.stroke)),
            Shape::QuadraticBezier(b) => self.path(&b.flatten(Some(0.1)), b.closed, b.fill, Self::path_stroke(&b.stroke)),
            Shape::Text(t) => self.text(t),
            Shape::Noop | Shape::Mesh(_) | Shape::Callback(_) => {}
        }
    }

    fn ellipse(&mut self, c: Pos2, r: Vec2, fill: Color32, stroke: Stroke) {
        let stroke = Self::stroke_of(stroke);
        if colorref(fill).is_none() && stroke.is_none() {
            return;
        }
        self.set_pen(stroke);
        self.set_brush(Some(fill));
        self.rect_rec(EMR_ELLIPSE, Rect::from_center_size(c, r * 2.0), None);
    }

    /// Text: one record per row, at its baseline, each letter where egui placed it.
    fn text(&mut self, t: &epaint::TextShape) {
        let galley = &t.galley;
        let (sin, cos) = t.angle.sin_cos();
        let rot = |v: Vec2| Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
        let escapement = (-t.angle.to_degrees() * 10.0).round() as i32;
        for row in &galley.rows {
            let glyphs: Vec<&epaint::text::Glyph> = row.glyphs.iter().filter(|g| g.chr != '\n' && g.chr != '\r').collect();
            // runs of letters in one format (colour and font)
            let mut i = 0;
            while i < glyphs.len() {
                let sec = glyphs[i].section_index;
                let mut j = i;
                while j < glyphs.len() && glyphs[j].section_index == sec {
                    j += 1;
                }
                let run = &glyphs[i..j];
                i = j;
                if run.iter().all(|g| g.chr.is_whitespace()) {
                    continue;
                }
                let Some(section) = galley.job.sections.get(sec as usize) else { continue };
                let fmt = &section.format;
                let mut color = t.override_text_color.unwrap_or(if fmt.color == Color32::PLACEHOLDER { t.fallback_color } else { fmt.color });
                if t.opacity_factor < 1.0 {
                    color = color.gamma_multiply(t.opacity_factor);
                }
                let Some(cref) = colorref(color) else { continue };
                let cjk = run.iter().any(|g| matches!(g.chr as u32, 0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF));
                let face = match &fmt.font_id.family {
                    egui::FontFamily::Name(n) => n.to_string(),
                    egui::FontFamily::Monospace => self.monospace_face.clone(),
                    egui::FontFamily::Proportional if cjk => self.cjk_face.clone(),
                    egui::FontFamily::Proportional => self.proportional_face.clone(),
                };
                self.set_font(&face, (fmt.font_id.size * K).round() as i32, escapement);
                if self.text_color != Some(cref) {
                    self.rec_u32(EMR_SETTEXTCOLOR, cref);
                    self.text_color = Some(cref);
                }
                let start = t.pos + rot(run[0].pos.to_vec2());
                let reference = self.pt(start);
                let mut chars: Vec<u16> = vec![];
                let mut dx: Vec<i32> = vec![];
                let mut x_done = (run[0].pos.x * K).round() as i32;
                for (k, g) in run.iter().enumerate() {
                    let next = run.get(k + 1).map(|n| n.pos.x).unwrap_or(g.pos.x + g.advance_width);
                    let next = (next * K).round() as i32;
                    let mut buf = [0u16; 2];
                    let units = g.chr.encode_utf16(&mut buf);
                    for (u, unit) in units.iter().enumerate() {
                        chars.push(*unit);
                        dx.push(if u == 0 { next - x_done } else { 0 });
                    }
                    x_done = next;
                }
                let end = t.pos + rot(Vec2::new(run[run.len() - 1].max_x(), run[0].pos.y));
                let far = self.pt(end);
                let asc = (fmt.font_id.size * K) as i32;
                let (bx0, bx1) = (reference.0.min(far.0), reference.0.max(far.0));
                let (by0, by1) = (reference.1.min(far.1) - asc, reference.1.max(far.1) + asc / 3);
                let n = chars.len() as u32;
                let off_string = 76u32;
                let str_bytes = (n * 2).div_ceil(4) * 4;
                let mut d = vec![];
                for v in [bx0, by0, bx1, by1] {
                    lei(&mut d, v);
                }
                le(&mut d, 1); // GM_COMPATIBLE
                d.extend_from_slice(&1.0f32.to_le_bytes());
                d.extend_from_slice(&1.0f32.to_le_bytes());
                // EMRTEXT
                lei(&mut d, reference.0);
                lei(&mut d, reference.1);
                le(&mut d, n);
                le(&mut d, off_string);
                le(&mut d, 0); // no options
                for v in [0, 0, -1, -1] {
                    lei(&mut d, v);
                }
                le(&mut d, off_string + str_bytes);
                for u in &chars {
                    d.extend_from_slice(&u.to_le_bytes());
                }
                while (d.len() + 8) % 4 != 0 {
                    d.push(0);
                }
                for v in &dx {
                    lei(&mut d, *v);
                }
                self.rec(EMR_EXTTEXTOUTW, &d);
            }
        }
    }

    /// The finished file: a header for a picture `size` diagram units big, the records, the end.
    pub fn finish(mut self, size: Vec2) -> Vec<u8> {
        let mut eof = vec![];
        le(&mut eof, 0);
        le(&mut eof, 16);
        le(&mut eof, 20);
        self.rec(EMR_EOF, &eof);
        let (w, h) = (((size.x * K).ceil() as i32).max(1), ((size.y * K).ceil() as i32).max(1));
        let bounds = self.bounds.unwrap_or((0, 0, w - 1, h - 1));
        // the frame (hundredths of a millimetre) is the whole picture, so pasted it keeps its size
        let hmm = |u: i32| (u as f32 * 2540.0 / DPI).round() as i32;
        let dev = (7680i32, 4320i32);
        let mm = ((dev.0 as f32 * 25.4 / DPI).round() as i32, (dev.1 as f32 * 25.4 / DPI).round() as i32);
        let um = ((dev.0 as f32 * 25400.0 / DPI).round() as i32, (dev.1 as f32 * 25400.0 / DPI).round() as i32);
        let header_size = 108u32;
        let total = header_size as usize + self.body.len();
        let mut out = Vec::with_capacity(total);
        le(&mut out, EMR_HEADER);
        le(&mut out, header_size);
        for v in [bounds.0, bounds.1, bounds.2, bounds.3] {
            lei(&mut out, v);
        }
        for v in [0, 0, hmm(w) - 1, hmm(h) - 1] {
            lei(&mut out, v);
        }
        le(&mut out, 0x464D_4520); // " EMF"
        le(&mut out, 0x0001_0000);
        le(&mut out, total as u32);
        le(&mut out, self.records + 1);
        out.extend_from_slice(&4u16.to_le_bytes()); // handles: the table's slot 0, pen, brush, font
        out.extend_from_slice(&0u16.to_le_bytes());
        le(&mut out, 0); // no description
        le(&mut out, 0);
        le(&mut out, 0); // no palette
        lei(&mut out, dev.0);
        lei(&mut out, dev.1);
        lei(&mut out, mm.0);
        lei(&mut out, mm.1);
        le(&mut out, 0); // no pixel format
        le(&mut out, 0);
        le(&mut out, 0); // not OpenGL
        lei(&mut out, um.0);
        lei(&mut out, um.1);
        debug_assert_eq!(out.len(), header_size as usize);
        out.extend_from_slice(&self.body);
        out
    }

    /// Start a picture: transparent text background, text placed by its baseline.
    pub fn begin() -> Self {
        let mut e = Emf::new();
        e.rec_u32(EMR_SETBKMODE, 1); // TRANSPARENT
        e.rec_u32(EMR_SETTEXTALIGN, 24); // TA_BASELINE | TA_LEFT | TA_NOUPDATECP
        e
    }
}

/// How many shapes from the start of `list` are the pieces of one dashed line: short lines with
/// the same stroke, each starting a short gap past the last one's end and going on the same way,
/// none longer than the first by much (an arrow head after a dashed line is not one of them).
fn dash_run(list: &[&Shape], first: [Pos2; 2], stroke: Stroke) -> usize {
    let len0 = (first[1] - first[0]).length();
    if !(0.5..=20.0).contains(&len0) {
        return 1;
    }
    let mut n = 1;
    let mut gap0: Option<f32> = None;
    let mut prev = first;
    for s in &list[1..] {
        let Shape::LineSegment { points: q, stroke: s2 } = s else { break };
        if *s2 != stroke {
            break;
        }
        let along = prev[1] - prev[0];
        let gap = q[0] - prev[1];
        let piece = q[1] - q[0];
        let (gl, pl) = (gap.length(), piece.length());
        if gl > 12.0 || pl > len0 * 1.6 + 0.5 || pl < 0.05 {
            break;
        }
        let a = along.normalized();
        if gl < 0.3 {
            // a dash going on round a corner: two pieces that touch; it must not turn back
            // (an arrow head's line from the end of a dashed line does)
            if piece.normalized().dot(a) < -0.2 {
                break;
            }
        } else {
            if let Some(g0) = gap0 {
                if (gl - g0).abs() > g0 * 0.6 + 0.5 {
                    break;
                }
            }
            // going on the same way (it may bend, along a curve or round a corner, but not turn back)
            if gap.normalized().dot(a) < 0.2 || piece.normalized().dot(gap.normalized()) < -0.2 {
                break;
            }
            gap0.get_or_insert(gl);
        }
        prev = *q;
        n += 1;
    }
    n
}

/// A line's points without the ones in the middle of a straight stretch.
fn simplify(pts: &[Pos2]) -> Vec<Pos2> {
    let mut out: Vec<Pos2> = vec![];
    for &p in pts {
        if let Some(&last) = out.last() {
            if (p - last).length() < 0.01 {
                continue;
            }
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let (u, v) = (b - a, p - b);
            if (u.x * v.y - u.y * v.x).abs() <= 0.02 * u.length() * v.length() && u.dot(v) > 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Put an EMF on the clipboard (as CF_ENHMETAFILE), replacing what is there.
#[cfg(windows)]
pub fn to_clipboard(bytes: &[u8]) -> Result<(), String> {
    use windows_sys::Win32::Graphics::Gdi::{DeleteEnhMetaFile, SetEnhMetaFileBits};
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    const CF_ENHMETAFILE: u32 = 14;
    unsafe {
        let h = SetEnhMetaFileBits(bytes.len() as u32, bytes.as_ptr());
        if h.is_null() {
            return Err("the picture could not be made".into());
        }
        // another program may hold the clipboard for a moment
        let mut open = false;
        for _ in 0..10 {
            if OpenClipboard(std::ptr::null_mut()) != 0 {
                open = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !open {
            DeleteEnhMetaFile(h);
            return Err("the clipboard is busy".into());
        }
        EmptyClipboard();
        let r = SetClipboardData(CF_ENHMETAFILE, h as _);
        CloseClipboard();
        if r.is_null() {
            DeleteEnhMetaFile(h);
            return Err("the clipboard refused the picture".into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn to_clipboard(_bytes: &[u8]) -> Result<(), String> {
    Err("editable pictures (EMF) can only be copied on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(b: &[u8]) -> Vec<(u32, usize)> {
        let mut v = vec![];
        let mut i = 0;
        while i + 8 <= b.len() {
            let t = u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
            let s = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
            assert!(s >= 8 && s % 4 == 0 && i + s <= b.len(), "record {t} at {i} has a bad size {s}");
            v.push((t, s));
            i += s;
        }
        assert_eq!(i, b.len());
        v
    }

    #[test]
    fn shapes_become_emf_records() {
        let mut e = Emf::begin();
        e.add_shape(&Shape::rect_filled(Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(100.0, 40.0)), 8.0, Color32::LIGHT_BLUE));
        e.add_shape(&Shape::circle_stroke(Pos2::new(50.0, 80.0), 10.0, Stroke::new(1.0, Color32::BLACK)));
        e.add_shape(&Shape::line_segment([Pos2::new(0.0, 0.0), Pos2::new(30.0, 30.0)], Stroke::new(1.5, Color32::RED)));
        e.add_shape(&Shape::convex_polygon(vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), Pos2::new(5.0, 8.0)], Color32::WHITE, Stroke::new(1.0, Color32::BLACK)));
        let b = e.finish(Vec2::new(120.0, 100.0));
        let r = records(&b);
        assert_eq!(r[0], (EMR_HEADER, 108));
        assert_eq!(r.last().unwrap().0, EMR_EOF);
        // the header counts every byte and record
        assert_eq!(u32::from_le_bytes(b[48..52].try_into().unwrap()) as usize, b.len());
        assert_eq!(u32::from_le_bytes(b[52..56].try_into().unwrap()) as usize, r.len());
        let kinds: Vec<u32> = r.iter().map(|x| x.0).collect();
        for k in [EMR_ROUNDRECT, EMR_ELLIPSE, EMR_POLYLINE, EMR_POLYGON] {
            assert!(kinds.contains(&k), "no record {k}");
        }
        // the frame is the picture's size: 120 diagram units = 1.25 inch = 3175 hundredths of a mm
        assert_eq!(i32::from_le_bytes(b[32..36].try_into().unwrap()), 3175 - 1);
    }

    #[test]
    fn a_dashed_line_stays_one_line() {
        let stroke = Stroke::new(1.0, Color32::BLACK);
        let mut shapes = Shape::dashed_line(&[Pos2::new(0.0, 0.0), Pos2::new(100.0, 0.0), Pos2::new(100.0, 60.0)], stroke, 6.0, 4.0);
        // an open arrow head at the end: two lines from the tip, kept as they are
        shapes.push(Shape::line_segment([Pos2::new(100.0, 60.0), Pos2::new(94.0, 50.0)], stroke));
        shapes.push(Shape::line_segment([Pos2::new(100.0, 60.0), Pos2::new(106.0, 50.0)], stroke));
        let mut e = Emf::begin();
        e.add_shapes(shapes.iter());
        let b = e.finish(Vec2::new(110.0, 70.0));
        let r = records(&b);
        let lines: Vec<usize> = r.iter().filter(|x| x.0 == EMR_POLYLINE).map(|x| (x.1 - 28) / 8).collect();
        // one dashed line round the corner (3 points), then the arrow head's two lines
        assert_eq!(lines, vec![3, 2, 2]);
        assert_eq!(r.iter().filter(|x| x.0 == EMR_EXTCREATEPEN).count(), 1);
    }
}
