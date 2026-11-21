//! Drawing shapes, connectors and toolbox icons with the egui painter.

use crate::model::{ConnKind, Connector, DiagramKind, End, ErdNotation, ProjectOptions, Shape, ShapeKind};
use crate::palette::Tool;
use egui::epaint;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::f32::consts::PI;

/// DefaultShapeBackground of the original (v.ddg.r.<clinit>: new Color(122, 207, 245)).
#[allow(dead_code)]
pub const DEFAULT_SHAPE_BACKGROUND: (u8, u8, u8) = (122, 207, 245);

/// Maps diagram coordinates to screen coordinates.
#[derive(Clone, Copy)]
pub struct Cam {
    pub origin: Pos2,
    pub pan: Vec2,
    pub zoom: f32,
}

impl Cam {
    pub fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.origin + self.pan + Vec2::new(x, y) * self.zoom
    }
    pub fn to_diagram(&self, p: Pos2) -> (f32, f32) {
        let v = (p - self.origin - self.pan) / self.zoom;
        (v.x, v.y)
    }
    pub fn rect(&self, s: &Shape) -> Rect {
        Rect::from_min_size(self.pt(s.x, s.y), Vec2::new(s.w, s.h) * self.zoom)
    }
}

/// Colours and sizes used for drawing.
#[derive(Clone, Copy)]
pub struct Look {
    pub dark: bool,
    pub text_size: f32,
    /// Drawing a toolbox icon: no text at all.
    pub icon: bool,
    /// Font family for diagram text (a registered name), or None for the built-in font.
    pub family: Option<&'static str>,
    /// Project colours: canvas, text, and the fill of new shapes.
    pub bg: Color32,
    pub ink: Color32,
    pub shape_fill: Color32,
}

static FONT_NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());

/// Mark a font family as ready to draw with (its fonts are loaded in egui).
pub fn register_family(name: &str) {
    let mut v = FONT_NAMES.lock().unwrap();
    if !v.iter().any(|n| *n == name) {
        v.push(Box::leak(name.to_string().into_boxed_str()));
    }
}

/// The registered family with this name, if its fonts are loaded.
pub fn family_ref(name: &str) -> Option<&'static str> {
    FONT_NAMES.lock().unwrap().iter().copied().find(|n| *n == name)
}

/// The look used for one shape: its own font and size, when set.
pub fn shape_look(shape: &Shape, look: &Look) -> Look {
    let mut l = *look;
    if let Some(sz) = shape.font_size {
        l.text_size = sz.clamp(4.0, 72.0);
    }
    if let Some(f) = &shape.font {
        if let Some(r) = family_ref(f) {
            l.family = Some(r);
        }
    }
    l
}

pub fn font_id(look: &Look, size: f32) -> FontId {
    match look.family {
        Some(n) => FontId::new(size.max(1.0), egui::FontFamily::Name(n.into())),
        None => FontId::proportional(size.max(1.0)),
    }
}

impl Look {
    /// Shape outline: DefaultShapeForeground is Color.BLACK (v.ddg.r.<clinit> bV -> field Iw).
    pub fn line(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(200, 205, 215)
        } else {
            Color32::BLACK
        }
    }
    /// Text: DefaultFontColor is black (v.ddg.r.<clinit> ad -> field HG).
    pub fn text(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(230, 232, 236)
        } else {
            self.ink
        }
    }
    /// Canvas: DefaultDiagramBackground is white (v.ddg.r.<clinit> T -> field HB).
    pub fn background(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(36, 38, 42)
        } else {
            self.bg
        }
    }
    pub fn solid(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(220, 224, 230)
        } else {
            Color32::BLACK
        }
    }
    pub fn accent(&self) -> Color32 {
        Color32::from_rgb(0, 120, 215)
    }
    /// Default fill of a shape kind. Every shape starts with DefaultShapeBackground
    /// = RGB(122, 207, 245) (v.ddg.r.<clinit> bT -> field Iv, setter f(Color)).
    /// Big frames (system, fragments, swimlanes) and stick figures stay unfilled (estimated).
    pub fn fill(&self, kind: ShapeKind) -> Color32 {
        use ShapeKind::*;
        let (r, g, b) = match kind {
            Constraint | System | AltFragment | LoopFragment | Actor | ActorLifeline | VerticalSwimlane | HorizontalSwimlane | NetworkZone => (self.bg.r(), self.bg.g(), self.bg.b()),
            // network pictures get soft colours of their own
            Firewall | WebFirewall => (254, 222, 212),
            Cloud => (236, 243, 252),
            // mind maps: a warm central topic, pale blue topics
            CentralTopic => (255, 214, 150),
            Topic => (226, 239, 255),
            Router | Switch | Hub | Server | LoadBalancer | Pc | Laptop | Phone | AccessPoint | Storage | Database | Person => (214, 232, 252),
            _ => (self.shape_fill.r(), self.shape_fill.g(), self.shape_fill.b()),
        };
        if self.dark {
            Color32::from_rgb((r as f32 * 0.28) as u8 + 20, (g as f32 * 0.28) as u8 + 22, (b as f32 * 0.28) as u8 + 28)
        } else {
            Color32::from_rgb(r, g, b)
        }
    }
    pub fn header_fill(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(52, 74, 100)
        } else {
            // entity/swimlane header a shade darker than the shape fill (estimated)
            let k = |v: u8| (v as f32 * 0.86) as u8;
            Color32::from_rgb(k(self.shape_fill.r()), k(self.shape_fill.g()), k(self.shape_fill.b()))
        }
    }
}

pub fn line_height(look: &Look) -> f32 {
    look.text_size * 1.4
}

fn font(look: &Look, cam: &Cam, scale: f32) -> FontId {
    font_id(look, look.text_size * scale * cam.zoom)
}

fn text_width_f(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x)
}

/// The keyword shown above a shape's name («interface»...), or the user's stereotype.
pub fn keyword(shape: &Shape, opts: &ProjectOptions) -> Option<String> {
    if !shape.stereotype.trim().is_empty() {
        return if opts.show_stereotypes { Some(format!("«{}»", shape.stereotype.trim())) } else { None };
    }
    use ShapeKind::*;
    let k = match shape.kind {
        Interface => "interface",
        Enumeration => "enumeration",
        Subsystem => "subsystem",
        Device => "device",
        ExecutionEnvironment => "execution environment",
        Artifact => "artifact",
        DeploymentSpecification => "deployment spec",
        DbView => "view",
        _ => return None,
    };
    Some(format!("«{k}»"))
}

fn visible_attributes(shape: &Shape, opts: &ProjectOptions) -> bool {
    match shape.kind {
        ShapeKind::Class | ShapeKind::Interface => opts.show_attributes && !shape.hide_attributes,
        k => k.has_attributes(),
    }
}

fn visible_operations(shape: &Shape, opts: &ProjectOptions) -> bool {
    shape.kind.has_operations() && opts.show_operations && !shape.hide_operations
}

/// Smallest size (diagram units) that fits the shape's text.
pub fn min_size(ctx: &egui::Context, shape: &Shape, opts: &ProjectOptions, look: &Look) -> Vec2 {
    use ShapeKind::*;
    let look = &shape_look(shape, look);
    let text_width = |ctx: &egui::Context, t: &str, size: f32| text_width_f(ctx, t, font_id(look, size));
    let ts = look.text_size;
    let lh = line_height(look);
    match shape.kind {
        Class | Interface | Enumeration | Instance | Entity | DbView | State => {
            let mut w = text_width(ctx, &shape.name, ts) + 24.0;
            let mut h = 8.0 + lh;
            if let Some(k) = keyword(shape, opts) {
                w = w.max(text_width(ctx, &k, ts * 0.9) + 24.0);
                h += lh * 0.9;
            }
            if shape.kind == Entity || shape.kind == DbView {
                h += 4.0;
            }
            if visible_attributes(shape, opts) {
                for a in &shape.attributes {
                    let key = shape.kind == Entity || shape.kind == DbView;
                    w = w.max(text_width(ctx, a, ts) + 24.0 + if key { 14.0 } else { 0.0 });
                }
                h += 8.0 + lh * shape.attributes.len().max(if shape.kind == State { 0 } else { 1 }) as f32;
            }
            if visible_operations(shape, opts) {
                for o in &shape.operations {
                    w = w.max(text_width(ctx, o, ts) + 24.0);
                }
                h += 8.0 + lh * shape.operations.len().max(1) as f32;
            }
            Vec2::new(w.max(60.0), h)
        }
        Component | Artifact | DeploymentSpecification | Action | ObjectNode | SendSignal | AcceptEvent => {
            let mut w = text_width(ctx, &shape.name, ts) + 44.0;
            if let Some(k) = keyword(shape, opts) {
                w = w.max(text_width(ctx, &k, ts * 0.9) + 44.0);
            }
            Vec2::new(w, lh * 2.0 + 8.0)
        }
        UseCase => Vec2::new(text_width(ctx, &shape.name, ts).min(220.0) * 1.2 + 30.0, lh * 2.5),
        Text => {
            // as wide as the longest line and as tall as all its lines (line breaks kept)
            let lines: Vec<&str> = shape.name.split('\n').collect();
            let w = lines.iter().map(|l| text_width(ctx, l, ts)).fold(0.0, f32::max);
            Vec2::new((w + 12.0).max(20.0), lines.len().max(1) as f32 * lh + 8.0)
        }
        Lifeline | ActorLifeline => Vec2::new(text_width(ctx, &shape.name, ts) + 24.0, 120.0),
        CentralTopic => Vec2::new(text_width(ctx, &shape.name, ts * 1.2) + 40.0, lh * 1.2 + 18.0),
        Topic => Vec2::new(text_width(ctx, &shape.name, ts) + 24.0, lh + 12.0),
        _ => Vec2::new(8.0, 8.0),
    }
}

/// The outline of a rounded rectangle as a closed list of points.
fn rounded_rect_points(r: Rect, rad: f32) -> Vec<Pos2> {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let mut v = vec![];
    let corners = [(r.right() - rad, r.top() + rad, -0.5 * PI), (r.right() - rad, r.bottom() - rad, 0.0), (r.left() + rad, r.bottom() - rad, 0.5 * PI), (r.left() + rad, r.top() + rad, PI)];
    for (cx, cy, a0) in corners {
        for i in 0..=6 {
            let a = a0 + i as f32 / 6.0 * 0.5 * PI;
            v.push(Pos2::new(cx + rad * a.cos(), cy + rad * a.sin()));
        }
    }
    v.push(v[0]);
    v
}

/// A line with a small arrow head at its end.
fn arrow_line(painter: &Painter, a: Pos2, b: Pos2, stroke: Stroke, head: f32) {
    painter.line_segment([a, b], stroke);
    let d = (b - a).normalized();
    let n = Vec2::new(-d.y, d.x);
    let p1 = b - d * head + n * head * 0.6;
    let p2 = b - d * head - n * head * 0.6;
    painter.add(epaint::Shape::convex_polygon(vec![b, p1, p2], stroke.color, Stroke::NONE));
}

/// A cylinder (router body, storage): top and bottom ellipses joined by straight sides.
fn cylinder(painter: &Painter, r: Rect, eh: f32, fill: Color32, stroke: Stroke) {
    let rx = r.width() / 2.0;
    let ry = eh / 2.0;
    let top_c = Pos2::new(r.center().x, r.top() + ry);
    let bot_c = Pos2::new(r.center().x, r.bottom() - ry);
    // body: lower half of the bottom ellipse, the sides and the top ellipse
    let mut body = vec![Pos2::new(r.left(), top_c.y)];
    for i in 0..=24 {
        let a = PI - i as f32 / 24.0 * PI;
        body.push(Pos2::new(bot_c.x + rx * a.cos(), bot_c.y + ry * a.sin()));
    }
    body.push(Pos2::new(r.right(), top_c.y));
    painter.add(epaint::Shape::convex_polygon(body.clone(), fill, Stroke::NONE));
    painter.add(epaint::Shape::line(body, stroke));
    let top = ellipse_points(top_c, rx, ry, 40);
    painter.add(epaint::Shape::convex_polygon(top, fill, stroke));
}

/// A flat box seen slightly from above (switch, hub): returns its front face.
fn slab(painter: &Painter, r: Rect, fill: Color32, stroke: Stroke) -> Rect {
    let depth = r.height() * 0.32;
    let skew = r.width() * 0.12;
    let front = Rect::from_min_max(Pos2::new(r.left(), r.top() + depth), Pos2::new(r.right() - skew, r.bottom()));
    let top = vec![Pos2::new(r.left(), front.top()), Pos2::new(r.left() + skew, r.top()), Pos2::new(r.right(), r.top()), Pos2::new(front.right(), front.top())];
    let side = vec![Pos2::new(front.right(), front.top()), Pos2::new(r.right(), r.top()), Pos2::new(r.right(), r.bottom() - depth), Pos2::new(front.right(), front.bottom())];
    painter.add(epaint::Shape::convex_polygon(top, fill.gamma_multiply(0.85), stroke));
    painter.add(epaint::Shape::convex_polygon(side, fill.gamma_multiply(0.7), stroke));
    painter.rect(front, 0.0, fill, stroke, StrokeKind::Middle);
    front
}

/// The picture of a network device, drawn to fill r.
fn draw_device(painter: &Painter, kind: ShapeKind, r: Rect, fill: Color32, stroke: Stroke, z: f32) {
    use ShapeKind::*;
    let ink = stroke.color;
    let thin = Stroke::new((stroke.width * 0.9).max(0.6), ink);
    let fill = if fill.a() == 0 { Color32::WHITE } else { fill };
    match kind {
        Router => {
            let eh = r.height() * 0.42;
            cylinder(painter, r, eh, fill, stroke);
            // four arrows on the top: two in, two out
            let c = Pos2::new(r.center().x, r.top() + eh / 2.0);
            let (dx, dy) = (r.width() * 0.32, eh * 0.32);
            let head = (3.0 * z).max(1.5);
            arrow_line(painter, Pos2::new(c.x - dx, c.y - dy), Pos2::new(c.x - dx * 0.25, c.y - dy * 0.25), thin, head);
            arrow_line(painter, Pos2::new(c.x + dx, c.y + dy), Pos2::new(c.x + dx * 0.25, c.y + dy * 0.25), thin, head);
            arrow_line(painter, Pos2::new(c.x + dx * 0.25, c.y - dy * 0.25), Pos2::new(c.x + dx, c.y - dy), thin, head);
            arrow_line(painter, Pos2::new(c.x - dx * 0.25, c.y + dy * 0.25), Pos2::new(c.x - dx, c.y + dy), thin, head);
        }
        Switch => {
            let f = slab(painter, r, fill, stroke);
            let head = (3.0 * z).max(1.5);
            let (l, rr) = (f.left() + f.width() * 0.18, f.right() - f.width() * 0.18);
            let y1 = f.top() + f.height() * 0.33;
            let y2 = f.top() + f.height() * 0.67;
            arrow_line(painter, Pos2::new(l, y1), Pos2::new(rr, y1), thin, head);
            arrow_line(painter, Pos2::new(rr, y2), Pos2::new(l, y2), thin, head);
        }
        Hub => {
            let f = slab(painter, r, fill, stroke);
            let n = 6;
            let w = f.width() / (n as f32 * 1.6 + 0.6);
            for i in 0..n {
                let x = f.left() + w * 0.6 + i as f32 * w * 1.6;
                let p = Rect::from_min_size(Pos2::new(x, f.center().y - w * 0.5), Vec2::splat(w));
                painter.rect_stroke(p, 0.0, thin, StrokeKind::Middle);
            }
        }
        Server => {
            painter.rect(r, 2.0 * z, fill, stroke, StrokeKind::Middle);
            for i in 0..3 {
                let y = r.top() + r.height() * (0.18 + i as f32 * 0.14);
                painter.line_segment([Pos2::new(r.left() + r.width() * 0.18, y), Pos2::new(r.right() - r.width() * 0.18, y)], thin);
            }
            painter.circle_filled(Pos2::new(r.right() - r.width() * 0.25, r.bottom() - r.height() * 0.14), (2.0 * z).max(1.0), Color32::from_rgb(60, 170, 90));
        }
        Firewall | WebFirewall => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            let rows = 4;
            let rh = r.height() / rows as f32;
            for i in 1..rows {
                let y = r.top() + rh * i as f32;
                painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], thin);
            }
            for i in 0..rows {
                let y0 = r.top() + rh * i as f32;
                let off = if i % 2 == 0 { 0.5 } else { 0.0 };
                let mut k = 0.0;
                while k < 3.0 {
                    let x = r.left() + r.width() * (k + off) / 3.0;
                    if x > r.left() + 1.0 && x < r.right() - 1.0 {
                        painter.line_segment([Pos2::new(x, y0), Pos2::new(x, y0 + rh)], thin);
                    }
                    k += 1.0;
                }
            }
            if kind == WebFirewall {
                // a globe on the wall's corner: web traffic
                let rad = r.height() * 0.3;
                let c = Pos2::new(r.right() - rad * 0.4, r.top() + rad * 0.4);
                painter.circle(c, rad, Color32::from_rgb(214, 232, 252), stroke);
                painter.add(epaint::Shape::closed_line(ellipse_points(c, rad * 0.45, rad, 24), thin));
                painter.line_segment([Pos2::new(c.x - rad, c.y), Pos2::new(c.x + rad, c.y)], thin);
            }
        }
        LoadBalancer => {
            painter.rect(r, 4.0 * z, fill, stroke, StrokeKind::Middle);
            let head = (3.0 * z).max(1.5);
            let a = Pos2::new(r.left() + r.width() * 0.14, r.center().y);
            let m = Pos2::new(r.left() + r.width() * 0.45, r.center().y);
            painter.line_segment([a, m], thin);
            for dy in [-0.28, 0.0, 0.28] {
                arrow_line(painter, m, Pos2::new(r.right() - r.width() * 0.14, r.center().y + r.height() * dy), thin, head);
            }
        }
        Pc => {
            let mon = Rect::from_min_max(r.min, Pos2::new(r.right(), r.top() + r.height() * 0.72));
            painter.rect(mon, 2.0 * z, fill, stroke, StrokeKind::Middle);
            painter.rect_stroke(mon.shrink(mon.height() * 0.12), 0.0, thin, StrokeKind::Middle);
            let neck_top = Pos2::new(r.center().x, mon.bottom());
            let base_y = r.bottom() - 1.0;
            painter.line_segment([neck_top, Pos2::new(r.center().x, base_y)], stroke);
            painter.line_segment([Pos2::new(r.center().x - r.width() * 0.22, base_y), Pos2::new(r.center().x + r.width() * 0.22, base_y)], stroke);
        }
        Laptop => {
            let scr = Rect::from_min_max(Pos2::new(r.left() + r.width() * 0.12, r.top()), Pos2::new(r.right() - r.width() * 0.12, r.top() + r.height() * 0.72));
            painter.rect(scr, 2.0 * z, fill, stroke, StrokeKind::Middle);
            painter.rect_stroke(scr.shrink(scr.height() * 0.12), 0.0, thin, StrokeKind::Middle);
            let base = vec![Pos2::new(scr.left(), scr.bottom()), Pos2::new(scr.right(), scr.bottom()), Pos2::new(r.right(), r.bottom()), Pos2::new(r.left(), r.bottom())];
            painter.add(epaint::Shape::convex_polygon(base, fill.gamma_multiply(0.85), stroke));
        }
        Phone => {
            painter.rect(r, 5.0 * z, fill, stroke, StrokeKind::Middle);
            let scr = Rect::from_min_max(Pos2::new(r.left() + r.width() * 0.14, r.top() + r.height() * 0.12), Pos2::new(r.right() - r.width() * 0.14, r.bottom() - r.height() * 0.2));
            painter.rect_stroke(scr, 1.0, thin, StrokeKind::Middle);
            painter.circle_stroke(Pos2::new(r.center().x, r.bottom() - r.height() * 0.1), (r.width() * 0.08).max(1.0), thin);
        }
        AccessPoint => {
            let bx = Rect::from_min_max(Pos2::new(r.left(), r.top() + r.height() * 0.6), r.max);
            painter.rect(bx, 4.0 * z, fill, stroke, StrokeKind::Middle);
            painter.circle_filled(Pos2::new(bx.right() - bx.width() * 0.18, bx.center().y), (1.8 * z).max(1.0), Color32::from_rgb(60, 170, 90));
            // wireless waves above the box
            let c = Pos2::new(r.center().x, bx.top() - 1.0);
            painter.circle_filled(c, (2.0 * z).max(1.0), ink);
            for k in 1..=3 {
                let rad = r.height() * 0.17 * k as f32;
                let pts: Vec<Pos2> = (0..=16).map(|i| {
                    let a = -PI * 0.78 + i as f32 / 16.0 * PI * 0.56;
                    Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin())
                }).collect();
                painter.add(epaint::Shape::line(pts, thin));
            }
        }
        Storage => {
            // a disk array: a cabinet of drive bays, each with a light
            painter.rect(r, 2.0 * z, fill, stroke, StrokeKind::Middle);
            let n = 4;
            let pad = r.width() * 0.12;
            let bay_h = (r.height() - pad * 2.0) / n as f32;
            for i in 0..n {
                let bay = Rect::from_min_size(Pos2::new(r.left() + pad, r.top() + pad + bay_h * i as f32), Vec2::new(r.width() - pad * 2.0, bay_h * 0.72));
                painter.rect_stroke(bay, 1.0, thin, StrokeKind::Middle);
                painter.circle_filled(Pos2::new(bay.right() - bay.height() * 0.5, bay.center().y), (bay.height() * 0.18).max(0.8), Color32::from_rgb(60, 170, 90));
            }
        }
        Database => {
            // the classic database drum: a cylinder with two bands
            let eh = r.height() * 0.26;
            cylinder(painter, r, eh, fill, stroke);
            let rx = r.width() / 2.0;
            let ry = eh / 2.0;
            for k in 1..=2 {
                let cy = r.top() + ry + (r.height() - eh) * k as f32 / 3.0;
                let pts: Vec<Pos2> = (0..=24).map(|i| {
                    let a = PI - i as f32 / 24.0 * PI;
                    Pos2::new(r.center().x + rx * a.cos(), cy + ry * a.sin())
                }).collect();
                painter.add(epaint::Shape::line(pts, stroke));
            }
        }
        Person => {
            // head and shoulders
            let head_r = r.width().min(r.height()) * 0.24;
            let head_c = Pos2::new(r.center().x, r.top() + head_r + 1.0);
            let body_top = head_c.y + head_r + r.height() * 0.06;
            let sw = r.width() / 2.0;
            let mut body = vec![];
            for i in 0..=20 {
                let a = PI + i as f32 / 20.0 * PI;
                body.push(Pos2::new(r.center().x + sw * a.cos(), body_top + (r.bottom() - body_top) * 0.55 + (r.bottom() - body_top) * 0.55 * a.sin()));
            }
            body.push(Pos2::new(r.right(), r.bottom()));
            body.push(Pos2::new(r.left(), r.bottom()));
            painter.add(epaint::Shape::convex_polygon(body, fill, stroke));
            painter.circle(head_c, head_r, fill, stroke);
        }
        _ => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
        }
    }
}

/// A cloud: overlapping ellipses, with only the outside parts of their edges drawn.
fn draw_cloud(painter: &Painter, r: Rect, fill: Color32, stroke: Stroke) {
    let parts = [(0.25, 0.6, 0.2), (0.45, 0.4, 0.24), (0.7, 0.42, 0.2), (0.8, 0.65, 0.17), (0.5, 0.68, 0.25)];
    let ell: Vec<(Pos2, f32, f32)> = parts.iter().map(|(x, y, rr)| (Pos2::new(r.left() + r.width() * x, r.top() + r.height() * y), r.width() * rr, r.height() * rr * 1.3)).collect();
    for (c, rx, ry) in &ell {
        painter.add(epaint::Shape::convex_polygon(ellipse_points(*c, *rx, *ry, 40), fill, Stroke::NONE));
    }
    let inside = |p: Pos2, skip: usize| ell.iter().enumerate().any(|(j, (c, rx, ry))| j != skip && ((p.x - c.x) / rx).powi(2) + ((p.y - c.y) / ry).powi(2) < 0.995);
    for (i, (c, rx, ry)) in ell.iter().enumerate() {
        let pts = ellipse_points(*c, *rx, *ry, 64);
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            if !inside(a, i) && !inside(b, i) {
                painter.line_segment([a, b], stroke);
            }
        }
    }
}

fn dashed(painter: &Painter, pts: &[Pos2], stroke: Stroke, zoom: f32) {
    let dash = (6.0 * zoom).max(2.0);
    let gap = (4.0 * zoom).max(1.5);
    for w in pts.windows(2) {
        painter.extend(epaint::Shape::dashed_line(&[w[0], w[1]], stroke, dash, gap));
    }
}

fn ellipse_points(c: Pos2, rx: f32, ry: f32, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * 2.0 * PI;
            Pos2::new(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

fn stick_figure(painter: &Painter, r: Rect, stroke: Stroke) {
    let w = r.width();
    let h = r.height();
    let cx = r.center().x;
    let head_r = (w.min(h * 0.5) * 0.28).max(1.5);
    let head_c = Pos2::new(cx, r.top() + head_r + h * 0.02);
    painter.circle_stroke(head_c, head_r, stroke);
    let neck = head_c.y + head_r;
    let hip = r.top() + h * 0.62;
    painter.line_segment([Pos2::new(cx, neck), Pos2::new(cx, hip)], stroke);
    let arm_y = neck + h * 0.12;
    painter.line_segment([Pos2::new(r.left() + w * 0.08, arm_y), Pos2::new(r.right() - w * 0.08, arm_y)], stroke);
    painter.line_segment([Pos2::new(cx, hip), Pos2::new(r.left() + w * 0.12, r.bottom())], stroke);
    painter.line_segment([Pos2::new(cx, hip), Pos2::new(r.right() - w * 0.12, r.bottom())], stroke);
}

fn text_at(painter: &Painter, look: &Look, pos: Pos2, align: Align2, text: &str, font: FontId, color: Color32) -> Rect {
    if look.icon || text.is_empty() {
        return Rect::from_min_size(pos, Vec2::ZERO);
    }
    painter.text(pos, align, text, font, color)
}

fn wrapped_center(painter: &Painter, look: &Look, r: Rect, text: &str, font: FontId, color: Color32) {
    if look.icon || text.is_empty() {
        return;
    }
    let galley = painter.layout(text.to_string(), font, color, (r.width() - 8.0).max(10.0));
    let size = galley.size();
    let pos = Pos2::new(r.center().x - size.x / 2.0, r.center().y - size.y / 2.0);
    // center each line
    let mut job_pos = pos;
    if galley.rows.len() > 1 {
        job_pos.x = r.center().x - size.x / 2.0;
    }
    painter.galley(job_pos, galley, color);
}

/// Draw one shape. `fill_override` is the user's colour.
pub fn draw_shape(painter: &Painter, cam: &Cam, shape: &Shape, look: &Look, opts: &ProjectOptions) {
    use ShapeKind::*;
    let r = cam.rect(shape);
    let z = cam.zoom;
    let line = look.line();
    let stroke = Stroke::new((1.2 * z).clamp(0.6, 2.5), line);
    let tc = look.text();
    let fill = shape.fill.map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])).unwrap_or_else(|| look.fill(shape.kind));
    let look = &shape_look(shape, look);
    let f = font(look, cam, 1.0);
    let fk = font(look, cam, 0.9);
    let lh = line_height(look) * z;
    // names drawn beside small shapes can be dragged away (label_offset)
    let off = Vec2::new(shape.label_offset.0, shape.label_offset.1) * z;
    let name_below = |painter: &Painter| {
        text_at(painter, look, Pos2::new(r.center().x, r.bottom() + 3.0 * z) + off, Align2::CENTER_TOP, &shape.name, f.clone(), tc);
    };

    match shape.kind {
        Class | Interface | Enumeration | Instance | Entity | DbView | State => {
            let radius = if shape.kind == State { 10.0 * z } else { 0.0 };
            painter.rect(r, radius, fill, stroke, StrokeKind::Middle);
            let mut y = r.top() + 4.0 * z;
            let is_entity = matches!(shape.kind, Entity | DbView);
            let kw = keyword(shape, opts);
            let header_h = 8.0 * z + lh + if kw.is_some() { lh * 0.9 } else { 0.0 } + if is_entity { 4.0 * z } else { 0.0 };
            let simple_state = shape.kind == State && shape.attributes.is_empty();
            if simple_state {
                // a simple state shows its name in the middle
                let block = lh + if kw.is_some() { lh * 0.9 } else { 0.0 };
                y = r.center().y - block / 2.0 - 1.0 * z;
            }
            if let Some(k) = &kw {
                text_at(painter, look, Pos2::new(r.center().x, y), Align2::CENTER_TOP, k, fk.clone(), tc);
                y += lh * 0.9;
            }
            let name = shape.name.clone();
            let nr = text_at(painter, look, Pos2::new(r.center().x, y + 1.0 * z), Align2::CENTER_TOP, &name, f.clone(), tc);
            if shape.kind == Instance && !look.icon && !name.is_empty() {
                painter.line_segment([Pos2::new(nr.left(), nr.bottom()), Pos2::new(nr.right(), nr.bottom())], Stroke::new(1.0 * z.max(0.7), tc));
            }
            let mut y = r.top() + header_h;
            let show_attr = visible_attributes(shape, opts);
            let show_ops = visible_operations(shape, opts);
            if show_attr && (shape.kind != State || !shape.attributes.is_empty()) {
                painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], stroke);
                y += 4.0 * z;
                for a in &shape.attributes {
                    let key = if is_entity { a.strip_prefix("PK ").map(|r| ("PK", r, Color32::from_rgb(200, 140, 0))).or_else(|| a.strip_prefix("FK ").map(|r| ("FK", r, Color32::from_rgb(40, 120, 200)))) } else { None };
                    if let Some((tag, rest, color)) = key {
                        text_at(painter, look, Pos2::new(r.left() + 6.0 * z, y + 1.5 * z), Align2::LEFT_TOP, tag, font(look, cam, 0.75), color);
                        text_at(painter, look, Pos2::new(r.left() + 26.0 * z, y), Align2::LEFT_TOP, rest, f.clone(), tc);
                    } else {
                        let x = if is_entity { r.left() + 26.0 * z } else { r.left() + 8.0 * z };
                        text_at(painter, look, Pos2::new(x, y), Align2::LEFT_TOP, a, f.clone(), tc);
                    }
                    y += lh;
                }
                if shape.attributes.is_empty() {
                    y += lh;
                }
                y += 4.0 * z;
            }
            if show_ops {
                painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], stroke);
                y += 4.0 * z;
                for o in &shape.operations {
                    text_at(painter, look, Pos2::new(r.left() + 8.0 * z, y), Align2::LEFT_TOP, o, f.clone(), tc);
                    y += lh;
                }
            }
        }
        Package | Subsystem => {
            let tab_w = (r.width() * 0.4).min(90.0 * z);
            let tab_h = 16.0 * z;
            let tab = Rect::from_min_size(r.min, Vec2::new(tab_w, tab_h));
            let body = Rect::from_min_max(Pos2::new(r.left(), r.top() + tab_h), r.max);
            painter.rect(tab, 0.0, fill, stroke, StrokeKind::Middle);
            painter.rect(body, 0.0, fill, stroke, StrokeKind::Middle);
            let mut y = body.top() + 6.0 * z;
            if let Some(k) = keyword(shape, opts) {
                text_at(painter, look, Pos2::new(body.center().x, y), Align2::CENTER_TOP, &k, fk.clone(), tc);
                y += lh * 0.9;
            }
            text_at(painter, look, Pos2::new(body.center().x, y), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
        }
        Text => {
            if look.icon {
                text_at(painter, &Look { icon: false, ..*look }, r.center(), Align2::CENTER_CENTER, "T", FontId::proportional(r.height() * 0.95), tc);
            } else {
                if shape.fill.is_some() {
                    painter.rect_filled(r, 0.0, fill);
                }
                let pad = 4.0 * z;
                let mut job = egui::text::LayoutJob::simple(shape.name.clone(), f.clone(), tc, (r.width() - 2.0 * pad).max(10.0));
                // alignment: 0 left (default), 1 centre, 2 right
                let (halign, x) = match shape.orient {
                    1 => (egui::Align::Center, r.center().x),
                    2 => (egui::Align::RIGHT, r.right() - pad),
                    _ => (egui::Align::LEFT, r.left() + pad),
                };
                job.halign = halign;
                let galley = painter.layout_job(job);
                painter.galley(Pos2::new(x, r.top() + pad), galley, tc);
                if shape.name.trim().is_empty() {
                    // an empty text box stays findable
                    let mut pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
                    pts.push(pts[0]);
                    dashed(painter, &pts, Stroke::new((1.0 * z).max(0.6), line.gamma_multiply(0.4)), z);
                }
            }
        }
        Note => {
            let fold = (12.0 * z).min(r.width() / 3.0).min(r.height() / 3.0);
            let pts = vec![
                r.left_top(),
                Pos2::new(r.right() - fold, r.top()),
                Pos2::new(r.right(), r.top() + fold),
                r.right_bottom(),
                r.left_bottom(),
            ];
            painter.add(epaint::Shape::convex_polygon(pts, fill, stroke));
            painter.add(epaint::Shape::line(
                vec![Pos2::new(r.right() - fold, r.top()), Pos2::new(r.right() - fold, r.top() + fold), Pos2::new(r.right(), r.top() + fold)],
                stroke,
            ));
            if !look.icon && !shape.name.is_empty() {
                let galley = painter.layout(shape.name.clone(), f.clone(), tc, (r.width() - 16.0 * z).max(10.0));
                painter.galley(Pos2::new(r.left() + 6.0 * z, r.top() + 6.0 * z), galley, tc);
            }
        }
        Constraint => {
            if look.icon {
                text_at(painter, &Look { icon: false, ..*look }, r.center(), Align2::CENTER_CENTER, "{ }", FontId::proportional(r.height() * 0.8), tc);
            } else {
                let t = format!("{{{}}}", shape.name);
                wrapped_center(painter, look, r, &t, f.clone(), tc);
            }
        }
        Collaboration => {
            let pts = ellipse_points(r.center(), r.width() / 2.0, r.height() / 2.0, 48);
            painter.add(epaint::Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
            let mut closed = pts;
            closed.push(closed[0]);
            dashed(painter, &closed, stroke, z);
            wrapped_center(painter, look, r, &shape.name, f.clone(), tc);
        }
        Component => {
            // a tall component usually holds others: a lighter shade keeps them apart
            let fill = if r.height() > 100.0 * z && shape.fill.is_none() { fill.lerp_to_gamma(look.background(), 0.55) } else { fill };
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            // component icon in the top right corner (can be hidden per shape)
            if !shape.hide_icon {
                let iw = 14.0 * z;
                let ih = 16.0 * z;
                let ir = Rect::from_min_size(Pos2::new(r.right() - iw - 8.0 * z, r.top() + 6.0 * z), Vec2::new(iw, ih));
                painter.rect(ir, 0.0, fill, Stroke::new(stroke.width * 0.8, line), StrokeKind::Middle);
                for k in 0..2 {
                    let ty = ir.top() + ih * (0.22 + 0.4 * k as f32);
                    let tr = Rect::from_min_size(Pos2::new(ir.left() - 4.0 * z, ty), Vec2::new(8.0 * z, 4.0 * z));
                    painter.rect(tr, 0.0, fill, Stroke::new(stroke.width * 0.8, line), StrokeKind::Middle);
                }
            }
            // a component holding other components shows its name at the top
            let tall = r.height() > 100.0 * z;
            let mut y = if tall { r.top() + 8.0 * z } else { r.center().y - lh / 2.0 };
            if let Some(k) = keyword(shape, opts) {
                if !tall {
                    y -= lh * 0.45;
                }
                text_at(painter, look, Pos2::new(r.center().x, y), Align2::CENTER_TOP, &k, fk.clone(), tc);
                y += lh * 0.9;
            }
            text_at(painter, look, Pos2::new(r.center().x, y), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
        }
        ProvidedInterface => {
            // the "ball" of a provided interface
            let rad = r.width().min(r.height()) / 2.0;
            painter.circle(r.center(), rad, fill, stroke);
            name_below(painter);
        }
        RequiredInterface => {
            // the "socket" of a required interface: a half circle open toward `orient`
            let rad = r.width().min(r.height()) / 2.0;
            let open = match shape.orient % 4 {
                0 => 0.0,
                1 => PI / 2.0,
                2 => PI,
                _ => -PI / 2.0,
            };
            let pts: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = open + PI / 2.0 + PI * i as f32 / 24.0;
                    Pos2::new(r.center().x + rad * a.cos(), r.center().y + rad * a.sin())
                })
                .collect();
            painter.add(epaint::Shape::line(pts, Stroke::new(stroke.width * 1.3, line)));
            name_below(painter);
        }
        Port => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            name_below(painter);
        }
        Node | Device | ExecutionEnvironment => {
            let d = (12.0 * z).min(r.width() / 4.0).min(r.height() / 4.0);
            let front = Rect::from_min_max(Pos2::new(r.left(), r.top() + d), Pos2::new(r.right() - d, r.bottom()));
            let top = vec![front.left_top(), Pos2::new(front.left() + d, r.top()), Pos2::new(r.right(), r.top()), front.right_top()];
            let side = vec![front.right_top(), Pos2::new(r.right(), r.top()), Pos2::new(r.right(), r.bottom() - d), front.right_bottom()];
            let shade = |c: Color32, k: f32| Color32::from_rgb((c.r() as f32 * k) as u8, (c.g() as f32 * k) as u8, (c.b() as f32 * k) as u8);
            painter.add(epaint::Shape::convex_polygon(top, shade(fill, 0.93), stroke));
            painter.add(epaint::Shape::convex_polygon(side, shade(fill, 0.85), stroke));
            painter.rect(front, 0.0, fill, stroke, StrokeKind::Middle);
            let mut y = front.top() + 6.0 * z;
            if let Some(k) = keyword(shape, opts) {
                text_at(painter, look, Pos2::new(front.center().x, y), Align2::CENTER_TOP, &k, fk.clone(), tc);
                y += lh * 0.9;
            }
            text_at(painter, look, Pos2::new(front.center().x, y), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
        }
        Artifact | DeploymentSpecification => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            let iw = 12.0 * z;
            let ih = 15.0 * z;
            let o = Pos2::new(r.right() - iw - 8.0 * z, r.top() + 6.0 * z);
            let fold = 4.0 * z;
            painter.add(epaint::Shape::closed_line(
                vec![o, Pos2::new(o.x + iw - fold, o.y), Pos2::new(o.x + iw, o.y + fold), Pos2::new(o.x + iw, o.y + ih), Pos2::new(o.x, o.y + ih)],
                Stroke::new(stroke.width * 0.8, line),
            ));
            let mut y = r.center().y - lh / 2.0;
            if let Some(k) = keyword(shape, opts) {
                y -= lh * 0.45;
                text_at(painter, look, Pos2::new(r.center().x, y), Align2::CENTER_TOP, &k, fk.clone(), tc);
                y += lh * 0.9;
            }
            text_at(painter, look, Pos2::new(r.center().x, y), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
        }
        Actor => {
            stick_figure(painter, r, stroke);
            name_below(painter);
        }
        k @ (Router | Switch | Hub | Server | Firewall | WebFirewall | LoadBalancer | Pc | Laptop | Phone | AccessPoint | Storage | Database | Person) => {
            draw_device(painter, k, r, fill, stroke, z);
            if !look.icon {
                name_below(painter);
            }
        }
        Cloud => {
            draw_cloud(painter, r, fill, stroke);
            if !look.icon {
                let c = Pos2::new(r.center().x, r.top() + r.height() * 0.62);
                text_at(painter, look, c, Align2::CENTER_CENTER, &shape.name, f.clone(), tc);
            }
        }
        CentralTopic => {
            // a pill with a thicker outline and a larger name
            painter.rect(r, (r.height() / 2.0).min(255.0), fill, Stroke::new(stroke.width * 1.8, line), StrokeKind::Middle);
            wrapped_center(painter, look, r.shrink2(Vec2::new(r.height() * 0.3, 0.0)), &shape.name, font(look, cam, 1.2), tc);
        }
        Topic => {
            painter.rect(r, (8.0 * z).min(r.height() / 2.0), fill, stroke, StrokeKind::Middle);
            wrapped_center(painter, look, r, &shape.name, f.clone(), tc);
        }
        NetworkZone => {
            let rad = 8.0 * z;
            painter.rect_filled(r, rad, fill);
            let pts = rounded_rect_points(r, rad);
            dashed(painter, &pts, Stroke::new(stroke.width, line), z);
            text_at(painter, look, Pos2::new(r.left() + 8.0 * z, r.top() + 5.0 * z), Align2::LEFT_TOP, &shape.name, f.clone(), tc);
        }
        UseCase => {
            let pts = ellipse_points(r.center(), r.width() / 2.0, r.height() / 2.0, 64);
            painter.add(epaint::Shape::convex_polygon(pts, fill, stroke));
            wrapped_center(painter, look, r.shrink2(Vec2::new(r.width() * 0.1, 0.0)), &shape.name, f.clone(), tc);
        }
        System => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            text_at(painter, look, Pos2::new(r.center().x, r.top() + 6.0 * z), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
        }
        Lifeline | ActorLifeline => {
            let head_h = if shape.kind == Lifeline { 40.0 * z } else { 70.0 * z };
            let head = Rect::from_min_size(r.min, Vec2::new(r.width(), head_h.min(r.height())));
            let cx = r.center().x;
            dashed(painter, &[Pos2::new(cx, head.bottom()), Pos2::new(cx, r.bottom())], Stroke::new(stroke.width, line), z);
            if shape.kind == Lifeline {
                painter.rect(head, 0.0, fill, stroke, StrokeKind::Middle);
                let lr = text_at(painter, look, head.center(), Align2::CENTER_CENTER, &shape.name, f.clone(), tc);
                let _ = lr;
            } else {
                let fig = Rect::from_center_size(Pos2::new(cx, head.top() + 24.0 * z), Vec2::new(26.0 * z, 46.0 * z));
                stick_figure(painter, fig, stroke);
                text_at(painter, look, Pos2::new(cx, fig.bottom() + 2.0 * z), Align2::CENTER_TOP, &shape.name, f.clone(), tc);
            }
        }
        AltFragment | LoopFragment => {
            painter.rect(r, 0.0, Color32::TRANSPARENT, stroke, StrokeKind::Middle);
            let label = if shape.kind == AltFragment { "alt" } else { "loop" };
            let tw = 40.0 * z;
            let th = 18.0 * z;
            let tab = vec![
                r.left_top(),
                Pos2::new(r.left() + tw, r.top()),
                Pos2::new(r.left() + tw, r.top() + th * 0.6),
                Pos2::new(r.left() + tw - th * 0.4, r.top() + th),
                Pos2::new(r.left(), r.top() + th),
            ];
            painter.add(epaint::Shape::convex_polygon(tab, fill, stroke));
            text_at(painter, look, Pos2::new(r.left() + 5.0 * z, r.top() + th / 2.0), Align2::LEFT_CENTER, label, font(look, cam, 0.95), tc);
            if !shape.name.is_empty() {
                text_at(painter, look, Pos2::new(r.left() + 8.0 * z, r.top() + th + 4.0 * z), Align2::LEFT_TOP, &format!("[{}]", shape.name), f.clone(), tc);
            }
            if shape.kind == AltFragment {
                // the sections, each with its guard under its line
                for dv in shape.alt_dividers() {
                    let my = r.top() + dv.y.clamp(crate::model::ALT_HEAD.min(shape.h), shape.h.max(0.0)) * z;
                    dashed(painter, &[Pos2::new(r.left(), my), Pos2::new(r.right(), my)], stroke, z);
                    if !dv.guard.trim().is_empty() {
                        text_at(painter, look, Pos2::new(r.left() + 8.0 * z, my + 4.0 * z), Align2::LEFT_TOP, &format!("[{}]", dv.guard.trim()), f.clone(), tc);
                    }
                }
            }
        }
        Initial => {
            painter.circle_filled(r.center(), r.width().min(r.height()) / 2.0, look.solid());
            name_below(painter);
        }
        Final | ActivityFinal => {
            let rad = r.width().min(r.height()) / 2.0;
            painter.circle(r.center(), rad, look.background(), Stroke::new(stroke.width, look.solid()));
            painter.circle_filled(r.center(), rad * 0.6, look.solid());
            name_below(painter);
        }
        FlowFinal | ExitPoint => {
            let rad = r.width().min(r.height()) / 2.0;
            painter.circle(r.center(), rad, look.background(), Stroke::new(stroke.width, look.solid()));
            let d = rad * 0.7071;
            let c = r.center();
            painter.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], Stroke::new(stroke.width, look.solid()));
            painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], Stroke::new(stroke.width, look.solid()));
            name_below(painter);
        }
        EntryPoint => {
            let rad = r.width().min(r.height()) / 2.0;
            painter.circle(r.center(), rad, look.background(), Stroke::new(stroke.width, look.solid()));
            name_below(painter);
        }
        Terminate => {
            let c = r.center();
            let d = r.width().min(r.height()) / 2.0;
            let st = Stroke::new(stroke.width * 1.4, look.solid());
            painter.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
            painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
            name_below(painter);
        }
        Choice | Decision | Merge => {
            let c = r.center();
            let pts = vec![Pos2::new(c.x, r.top()), Pos2::new(r.right(), c.y), Pos2::new(c.x, r.bottom()), Pos2::new(r.left(), c.y)];
            painter.add(epaint::Shape::convex_polygon(pts, look.background(), stroke));
            if !shape.name.is_empty() {
                text_at(painter, look, Pos2::new(r.right() + 4.0 * z, r.top()), Align2::LEFT_BOTTOM, &shape.name, f.clone(), tc);
            }
        }
        Junction => {
            painter.circle_filled(r.center(), r.width().min(r.height()) / 2.0, look.solid());
            name_below(painter);
        }
        Fork | Join => {
            painter.rect_filled(r, 1.0, look.solid());
            if !shape.name.is_empty() {
                text_at(painter, look, Pos2::new(r.right() + 4.0 * z, r.center().y), Align2::LEFT_CENTER, &shape.name, f.clone(), tc);
            }
        }
        ShallowHistory | DeepHistory => {
            let rad = r.width().min(r.height()) / 2.0;
            painter.circle(r.center(), rad, look.background(), Stroke::new(stroke.width, look.solid()));
            let t = if shape.kind == ShallowHistory { "H" } else { "H*" };
            text_at(painter, &Look { icon: false, ..*look }, r.center(), Align2::CENTER_CENTER, t, FontId::proportional(rad * 1.1), look.solid());
        }
        Action => {
            painter.rect(r, (12.0 * z).min(r.height() / 2.0), fill, stroke, StrokeKind::Middle);
            wrapped_center(painter, look, r, &shape.name, f.clone(), tc);
        }
        ObjectNode => {
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            wrapped_center(painter, look, r, &shape.name, f.clone(), tc);
        }
        SendSignal => {
            let p = (14.0 * z).min(r.width() / 3.0);
            let pts = vec![r.left_top(), Pos2::new(r.right() - p, r.top()), Pos2::new(r.right(), r.center().y), Pos2::new(r.right() - p, r.bottom()), r.left_bottom()];
            painter.add(epaint::Shape::convex_polygon(pts, fill, stroke));
            wrapped_center(painter, look, Rect::from_min_max(r.min, Pos2::new(r.right() - p, r.bottom())), &shape.name, f.clone(), tc);
        }
        AcceptEvent => {
            let p = (14.0 * z).min(r.width() / 3.0);
            let body = Rect::from_min_max(Pos2::new(r.left() + p, r.top()), r.max);
            painter.rect_filled(body, 0.0, fill);
            painter.add(epaint::Shape::convex_polygon(vec![r.left_top(), body.left_top(), body.left_bottom(), r.left_bottom()], Color32::TRANSPARENT, Stroke::NONE));
            painter.add(epaint::Shape::convex_polygon(vec![r.left_top(), body.left_top(), Pos2::new(r.left() + p, r.center().y)], fill, Stroke::NONE));
            painter.add(epaint::Shape::convex_polygon(vec![Pos2::new(r.left() + p, r.center().y), body.left_bottom(), r.left_bottom()], fill, Stroke::NONE));
            painter.add(epaint::Shape::closed_line(
                vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), Pos2::new(r.left() + p, r.center().y)],
                stroke,
            ));
            wrapped_center(painter, look, body, &shape.name, f.clone(), tc);
        }
        AcceptTimeEvent => {
            let pts = vec![r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()];
            painter.add(epaint::Shape::convex_polygon(vec![pts[0], pts[1], r.center()], fill, Stroke::NONE));
            painter.add(epaint::Shape::convex_polygon(vec![r.center(), pts[3], pts[2]], fill, Stroke::NONE));
            painter.add(epaint::Shape::closed_line(pts, stroke));
            name_below(painter);
        }
        VerticalSwimlane | HorizontalSwimlane => {
            // one swimlane: an optional header with its name, then its partitions side by side,
            // each with a header naming it and a line between neighbours
            let vertical = shape.kind == VerticalSwimlane;
            painter.rect(r, 0.0, fill, stroke, StrokeKind::Middle);
            let to_rect = |(x, y, w, h): (f32, f32, f32, f32)| Rect::from_two_pos(cam.pt(x, y), cam.pt(x + w, y + h));
            let label = |hr: Rect, text: &str| {
                if text.is_empty() || look.icon {
                    return;
                }
                if vertical {
                    text_at(painter, look, hr.center(), Align2::CENTER_CENTER, text, f.clone(), tc);
                } else {
                    let galley = painter.layout_no_wrap(text.to_string(), f.clone(), tc);
                    let sz = galley.size();
                    let pos = Pos2::new(hr.center().x - sz.y / 2.0, hr.center().y + sz.x / 2.0);
                    painter.add(epaint::TextShape::new(pos, galley, tc).with_angle(-PI / 2.0));
                }
            };
            if let Some(t) = shape.lane_title_rect() {
                let hr = to_rect(t);
                painter.rect(hr, 0.0, look.header_fill(), stroke, StrokeKind::Middle);
                label(hr, &shape.name);
            }
            let head0 = if shape.lane_titled() { crate::model::LANE_HEAD * z } else { 0.0 };
            for (i, (p, (at, _))) in shape.lane_partitions().iter().zip(shape.partition_spans()).enumerate() {
                if let Some(h) = shape.partition_header(i) {
                    let hr = to_rect(h);
                    painter.rect(hr, 0.0, look.header_fill(), stroke, StrokeKind::Middle);
                    label(hr, &p.name);
                }
                if i > 0 {
                    let a = at * z;
                    let seg = if vertical { [Pos2::new(r.left() + a, r.top() + head0), Pos2::new(r.left() + a, r.bottom())] } else { [Pos2::new(r.left() + head0, r.top() + a), Pos2::new(r.right(), r.top() + a)] };
                    painter.line_segment(seg, stroke);
                }
            }
        }
    }
}

/// Selection outline and resize handles.
pub fn draw_selection(painter: &Painter, cam: &Cam, shape: &Shape, look: &Look) {
    let r = cam.rect(shape).expand(3.0);
    let st = Stroke::new(1.0_f32, look.accent());
    dashed(painter, &[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()], st, 0.7);
    if !shape.kind.fixed_size() {
        for c in handle_points(cam, shape) {
            let hr = Rect::from_center_size(c, Vec2::splat(7.0));
            painter.rect(hr, 0.0, Color32::WHITE, Stroke::new(1.0_f32, look.accent()), StrokeKind::Middle);
        }
    }
}

/// Screen positions of the four resize handles (top-left, top-right, bottom-right, bottom-left).
pub fn handle_points(cam: &Cam, shape: &Shape) -> [Pos2; 4] {
    let r = cam.rect(shape).expand(3.0);
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
}

fn is_round(kind: ShapeKind) -> bool {
    use ShapeKind::*;
    matches!(kind, UseCase | Collaboration | ProvidedInterface | RequiredInterface | Initial | Final | ActivityFinal | FlowFinal | Junction | ShallowHistory | DeepHistory | EntryPoint | ExitPoint)
}

/// The point where a line from the shape's centre toward `to` leaves the shape.
pub fn boundary_point(s: &Shape, to: (f32, f32)) -> (f32, f32) {
    let (cx, cy) = s.center();
    let dx = to.0 - cx;
    let dy = to.1 - cy;
    if dx.abs() < 1e-3 && dy.abs() < 1e-3 {
        return (cx, cy);
    }
    let hw = s.w / 2.0;
    let hh = s.h / 2.0;
    if is_round(s.kind) {
        let a = dy.atan2(dx);
        return (cx + hw * a.cos(), cy + hh * a.sin());
    }
    if matches!(s.kind, ShapeKind::Choice | ShapeKind::Decision | ShapeKind::Merge) {
        let t = 1.0 / (dx.abs() / hw + dy.abs() / hh);
        return (cx + dx * t, cy + dy * t);
    }
    let tx = if dx.abs() > 1e-6 { hw / dx.abs() } else { f32::INFINITY };
    let ty = if dy.abs() > 1e-6 { hh / dy.abs() } else { f32::INFINITY };
    let t = tx.min(ty);
    (cx + dx * t, cy + dy * t)
}

/// Connector styles, with the original's codes (vpplatform.jar v.bjl.u, v.dxx.q.a).
pub const STYLE_RECTILINEAR: u8 = 0;
pub const STYLE_OBLIQUE: u8 = 1;
pub const STYLE_CURVE: u8 = 2;
pub const STYLE_FOLLOW_DIAGRAM: u8 = 3;
pub const STYLE_ROUND_OBLIQUE: u8 = 4;
pub const STYLE_ROUND_RECTILINEAR: u8 = 5;

/// Names in the original's order: the per-line list starts with "Follow diagram" (v.bjl.u.e).
pub const LINE_STYLES: [(u8, &str); 6] = [
    (STYLE_FOLLOW_DIAGRAM, "Follow diagram"),
    (STYLE_RECTILINEAR, "Rectilinear"),
    (STYLE_OBLIQUE, "Oblique"),
    (STYLE_CURVE, "Curve"),
    (STYLE_ROUND_RECTILINEAR, "Round rectilinear"),
    (STYLE_ROUND_OBLIQUE, "Round oblique"),
];

/// The style a connector is drawn with: its own, or the diagram default when "Follow diagram".
pub fn effective_style(c: &Connector, default_style: u8) -> u8 {
    if c.style == STYLE_FOLLOW_DIAGRAM {
        if default_style == STYLE_FOLLOW_DIAGRAM {
            STYLE_OBLIQUE
        } else {
            default_style
        }
    } else {
        c.style
    }
}

/// The connector's drawn path in diagram coordinates, routed with its style.
pub fn conn_path(dkind: DiagramKind, c: &Connector, from: &Shape, to: &Shape, default_style: u8) -> Vec<(f32, f32)> {
    let controls = conn_controls(dkind, c, from, to);
    if dkind == DiagramKind::Sequence && c.kind.is_message() {
        return controls;
    }
    if let Some(curve) = branch_curve(c, from, to) {
        return curve;
    }
    let style = effective_style(c, default_style);
    let rect = matches!(style, STYLE_RECTILINEAR | STYLE_ROUND_RECTILINEAR);
    let mut path = if rect && from.id != to.id { rectilinear(c, from, to) } else { controls };
    path.dedup_by(|a, b| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01);
    match style {
        STYLE_CURVE => smooth(&path),
        STYLE_ROUND_OBLIQUE | STYLE_ROUND_RECTILINEAR => round_corners(&path, 10.0),
        _ => path,
    }
}

/// A mind map branch left on "Follow diagram" with no bend points or fixed ends: a smooth S
/// curve from the side of the parent topic facing the child to the near side of the child,
/// leaving and arriving level. None when the topics overlap side to side (a straight line then).
fn branch_curve(c: &Connector, from: &Shape, to: &Shape) -> Option<Vec<(f32, f32)>> {
    if c.kind != ConnKind::Branch || c.style != STYLE_FOLLOW_DIAGRAM || !c.points.is_empty() || c.from_anchor.is_some() || c.to_anchor.is_some() || from.id == to.id {
        return None;
    }
    let right = to.center().0 >= from.center().0;
    let (sx, ex) = if right { (from.x + from.w, to.x) } else { (from.x, to.x + to.w) };
    if (ex - sx) * if right { 1.0 } else { -1.0 } < 4.0 {
        return None;
    }
    let (sy, ey) = (from.center().1, to.center().1);
    let k = (ex - sx) * 0.5;
    let n = 20;
    Some(
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                let u = 1.0 - t;
                let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                (b0 * sx + b1 * (sx + k) + b2 * (ex - k) + b3 * ex, b0 * sy + b1 * sy + b2 * ey + b3 * ey)
            })
            .collect(),
    )
}

/// Right-angle routing: through the bend points with horizontal/vertical runs only.
fn rectilinear(c: &Connector, from: &Shape, to: &Shape) -> Vec<(f32, f32)> {
    let fa = c.from_anchor.map(|f| anchor_point(from, f));
    let ta = c.to_anchor.map(|f| anchor_point(to, f));
    let (ac, bc) = (fa.unwrap_or(from.center()), ta.unwrap_or(to.center()));
    let mut out: Vec<(f32, f32)> = vec![ac];
    if c.points.is_empty() {
        // a Z (or straight) route through the middle; an end fixed on a left/right edge
        // leaves sideways, one on a top/bottom edge leaves up or down
        let sideways = match c.from_anchor {
            Some(f) => f.0 <= 0.001 || f.0 >= 0.999,
            None => (bc.0 - ac.0).abs() >= (bc.1 - ac.1).abs(),
        };
        if sideways {
            let mx = (ac.0 + bc.0) / 2.0;
            out.push((mx, ac.1));
            out.push((mx, bc.1));
        } else {
            let my = (ac.1 + bc.1) / 2.0;
            out.push((ac.0, my));
            out.push((bc.0, my));
        }
        out.push(bc);
    } else {
        let mut targets = c.points.clone();
        targets.push(bc);
        for q in targets {
            let p = *out.last().unwrap();
            if (p.0 - q.0).abs() > 0.01 && (p.1 - q.1).abs() > 0.01 {
                out.push((q.0, p.1));
            }
            out.push(q);
        }
    }
    // clip the free ends to the shapes' outlines
    let n = out.len();
    if fa.is_none() {
        out[0] = boundary_point(from, out[1]);
    }
    if ta.is_none() {
        out[n - 1] = boundary_point(to, out[n - 2]);
    }
    out
}

/// The point on a shape's outline an anchor (fractions of width and height) stands for.
pub fn anchor_point(s: &Shape, f: (f32, f32)) -> (f32, f32) {
    let p = (s.x + f.0 * s.w, s.y + f.1 * s.h);
    if is_round(s.kind) || matches!(s.kind, ShapeKind::Choice | ShapeKind::Decision | ShapeKind::Merge) {
        boundary_point(s, p)
    } else {
        p
    }
}

/// The anchor (fractions) of the point on a shape's outline nearest to `p`.
pub fn nearest_anchor(s: &Shape, p: (f32, f32)) -> (f32, f32) {
    let frac = |q: (f32, f32)| (((q.0 - s.x) / s.w.max(1.0)).clamp(0.0, 1.0), ((q.1 - s.y) / s.h.max(1.0)).clamp(0.0, 1.0));
    if is_round(s.kind) || matches!(s.kind, ShapeKind::Choice | ShapeKind::Decision | ShapeKind::Merge) {
        return frac(boundary_point(s, p));
    }
    let x = p.0.clamp(s.x, s.x + s.w);
    let y = p.1.clamp(s.y, s.y + s.h);
    let (dl, dr, dt, db) = ((x - s.x).abs(), (s.x + s.w - x).abs(), (y - s.y).abs(), (s.y + s.h - y).abs());
    let m = dl.min(dr).min(dt).min(db);
    if m == dl {
        (0.0, frac((x, y)).1)
    } else if m == dr {
        (1.0, frac((x, y)).1)
    } else if m == dt {
        (frac((x, y)).0, 0.0)
    } else {
        (frac((x, y)).0, 1.0)
    }
}

/// Lanes for lines between two side-by-side (or stacked) boxes: where the k-th of n lines
/// meets each box, so parallel lines stay apart. None when the boxes neither sit side by
/// side nor one above the other, or a box is not a plain box.
pub fn lane_anchors(a: &Shape, b: &Shape, k: usize, n: usize) -> Option<((f32, f32), (f32, f32))> {
    let boxy = |s: &Shape| s.kind.is_network_device() || !(is_round(s.kind) || s.kind.is_pseudo() || matches!(s.kind, ShapeKind::Choice | ShapeKind::Decision | ShapeKind::Merge));
    if !boxy(a) || !boxy(b) {
        return None;
    }
    let t = (k + 1) as f32 / (n + 1) as f32;
    if a.x + a.w <= b.x || b.x + b.w <= a.x {
        let (lo, hi) = (a.y.max(b.y), (a.y + a.h).min(b.y + b.h));
        if hi - lo < 8.0 {
            return None;
        }
        let y = lo + (hi - lo) * t;
        let a_left = a.x + a.w <= b.x;
        return Some(((if a_left { 1.0 } else { 0.0 }, (y - a.y) / a.h), (if a_left { 0.0 } else { 1.0 }, (y - b.y) / b.h)));
    }
    if a.y + a.h <= b.y || b.y + b.h <= a.y {
        let (lo, hi) = (a.x.max(b.x), (a.x + a.w).min(b.x + b.w));
        if hi - lo < 8.0 {
            return None;
        }
        let x = lo + (hi - lo) * t;
        let a_top = a.y + a.h <= b.y;
        return Some((((x - a.x) / a.w, if a_top { 1.0 } else { 0.0 }), ((x - b.x) / b.w, if a_top { 0.0 } else { 1.0 })));
    }
    None
}

/// A smooth curve through the points (Catmull-Rom, sampled).
fn smooth(p: &[(f32, f32)]) -> Vec<(f32, f32)> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut out = vec![p[0]];
    for i in 0..p.len() - 1 {
        let p0 = if i == 0 { p[0] } else { p[i - 1] };
        let (p1, p2) = (p[i], p[i + 1]);
        let p3 = if i + 2 < p.len() { p[i + 2] } else { p[i + 1] };
        for s in 1..=12 {
            let t = s as f32 / 12.0;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3);
            out.push((f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1)));
        }
    }
    out
}

/// Replace each inner corner with a short rounded bend.
fn round_corners(p: &[(f32, f32)], radius: f32) -> Vec<(f32, f32)> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut out = vec![p[0]];
    for i in 1..p.len() - 1 {
        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
        let l1 = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let l2 = ((c.0 - b.0).powi(2) + (c.1 - b.1).powi(2)).sqrt();
        let r = radius.min(l1 / 2.0).min(l2 / 2.0);
        if r < 0.5 {
            out.push(b);
            continue;
        }
        let s = (b.0 + (a.0 - b.0) * r / l1, b.1 + (a.1 - b.1) * r / l1);
        let e = (b.0 + (c.0 - b.0) * r / l2, b.1 + (c.1 - b.1) * r / l2);
        out.push(s);
        for k in 1..6 {
            let t = k as f32 / 6.0;
            let u = 1.0 - t;
            out.push((u * u * s.0 + 2.0 * u * t * b.0 + t * t * e.0, u * u * s.1 + 2.0 * u * t * b.1 + t * t * e.1));
        }
        out.push(e);
    }
    out.push(p[p.len() - 1]);
    out
}

/// The connector's control path: end points and bend points, before routing.
pub fn conn_controls(dkind: DiagramKind, c: &Connector, from: &Shape, to: &Shape) -> Vec<(f32, f32)> {
    if dkind == DiagramKind::Sequence && c.kind.is_message() && from.kind.is_lifeline() && to.kind.is_lifeline() {
        let x1 = from.x + from.w / 2.0;
        let x2 = to.x + to.w / 2.0;
        let y = c.y;
        if from.id == to.id {
            return vec![(x1, y), (x1 + 40.0, y), (x1 + 40.0, y + 22.0), (x1, y + 22.0)];
        }
        if c.kind == ConnKind::CreateMessage {
            // ends at the head of the created lifeline
            let edge = if x2 > x1 { to.x } else { to.x + to.w };
            return vec![(x1, y), (edge, y)];
        }
        return vec![(x1, y), (x2, y)];
    }
    if !c.points.is_empty() {
        // a bent line: outline (or its fixed end) -> bend points -> outline
        let first = c.points[0];
        let last = c.points[c.points.len() - 1];
        let mut v = vec![c.from_anchor.map(|f| anchor_point(from, f)).unwrap_or_else(|| boundary_point(from, first))];
        v.extend(c.points.iter().copied());
        v.push(c.to_anchor.map(|f| anchor_point(to, f)).unwrap_or_else(|| boundary_point(to, last)));
        return v;
    }
    if from.id == to.id {
        let r = from.x + from.w;
        let t = from.y;
        let d = 28.0;
        return vec![(r - from.w * 0.25, t), (r - from.w * 0.25, t - d), (r + d, t - d), (r + d, t + from.h * 0.25), (r, t + from.h * 0.25)];
    }
    // fixed ends sit where they were put; a free end points at the other end
    let fa = c.from_anchor.map(|f| anchor_point(from, f));
    let ta = c.to_anchor.map(|f| anchor_point(to, f));
    let a = fa.unwrap_or_else(|| boundary_point(from, ta.unwrap_or(to.center())));
    let b = ta.unwrap_or_else(|| boundary_point(to, fa.unwrap_or(from.center())));
    vec![a, b]
}

fn arrow_end(painter: &Painter, tip: Pos2, dir: Vec2, end: End, stroke: Stroke, look: &Look, z: f32, notation: ErdNotation) {
    let d = dir.normalized();
    let n = Vec2::new(-d.y, d.x);
    let l = 12.0 * z.max(0.4);
    let w = 6.0 * z.max(0.4);
    match end {
        End::None => {}
        End::OpenArrow => {
            painter.line_segment([tip, tip - d * l + n * w], stroke);
            painter.line_segment([tip, tip - d * l - n * w], stroke);
        }
        End::Triangle => {
            let pts = vec![tip, tip - d * l * 1.2 + n * w * 1.2, tip - d * l * 1.2 - n * w * 1.2];
            painter.add(epaint::Shape::convex_polygon(pts, look.background(), stroke));
        }
        End::FilledArrow => {
            let pts = vec![tip, tip - d * l + n * w, tip - d * l - n * w];
            painter.add(epaint::Shape::convex_polygon(pts, stroke.color, stroke));
        }
        End::HollowDiamond | End::FilledDiamond => {
            let fill = if end == End::FilledDiamond { stroke.color } else { look.background() };
            let pts = vec![tip, tip - d * l + n * w, tip - d * l * 2.0, tip - d * l - n * w];
            painter.add(epaint::Shape::convex_polygon(pts, fill, stroke));
        }
        End::CircleCross => {
            let r = 6.0 * z.max(0.4);
            let c = tip - d * r;
            painter.circle(c, r, look.background(), stroke);
            painter.line_segment([c - d * r, c + d * r], stroke);
            painter.line_segment([c - n * r, c + n * r], stroke);
        }
        End::CrowOne | End::CrowMany => {
            if notation == ErdNotation::Multiplicity {
                let t = if end == End::CrowOne { "1" } else { "*" };
                if !look.icon {
                    painter.text(tip - d * 10.0 * z + n * 9.0 * z, Align2::CENTER_CENTER, t, FontId::proportional((look.text_size * z).max(1.0)), look.text());
                }
                return;
            }
            let bar1 = tip - d * l * 0.8;
            painter.line_segment([bar1 + n * w, bar1 - n * w], stroke);
            if end == End::CrowOne {
                let bar2 = tip - d * l * 1.3;
                painter.line_segment([bar2 + n * w, bar2 - n * w], stroke);
            } else {
                let base = tip - d * l * 0.8;
                painter.line_segment([base, tip + n * w * 1.2], stroke);
                painter.line_segment([base, tip - n * w * 1.2], stroke);
                painter.line_segment([base, tip], stroke);
            }
        }
    }
}

/// Draw a connector; `number` is the message number in sequence diagrams.
pub fn draw_connector(
    painter: &Painter,
    cam: &Cam,
    dkind: DiagramKind,
    c: &Connector,
    _from: &Shape,
    to: &Shape,
    look: &Look,
    opts: &ProjectOptions,
    number: Option<&str>,
    path_d: &[(f32, f32)],
    hops: &[(usize, f32)],
    selected: bool,
) -> [Option<Rect>; 3] {
    let path: Vec<Pos2> = path_d.iter().map(|(x, y)| cam.pt(*x, *y)).collect();
    if path.len() < 2 {
        return [None; 3];
    }
    let z = cam.zoom;
    let color = if selected { look.accent() } else { look.line() };
    let stroke = Stroke::new((1.2 * z).clamp(0.7, 2.5) + if selected { 0.8 } else { 0.0 }, color);
    let mut style = c.kind.style();
    // the line to a provided-interface ball is a Realization drawn as a plain line
    // (PortProvidedInterfaceEdgeDrawEngine, vpplatform.jar v.dwx.b.a); the line to a
    // required-interface socket is a Usage drawn the same way (estimated)
    if (c.kind == ConnKind::Realization && to.kind == ShapeKind::ProvidedInterface) || (c.kind == ConnKind::Usage && to.kind == ShapeKind::RequiredInterface) {
        style = crate::model::ConnStyle { dashed: false, start: End::None, end: End::None, keyword: None };
    }
    // where this line crosses another, it jumps over it with a small arc
    let drawn = apply_hops(&path, hops, HOP_RADIUS * z.clamp(0.6, 2.0));
    if style.dashed {
        dashed(painter, &drawn, stroke, z);
    } else {
        painter.add(epaint::Shape::line(drawn, stroke));
    }
    let n = path.len();
    let end_dir = path[n - 1] - path[n - 2];
    let start_dir = path[0] - path[1];
    let solid = Stroke::new(stroke.width, color);
    // navigability arrows chosen by the user (association, aggregation, composition)
    let end_kind = if shows_arrow(c, true) && style.end == End::None { End::OpenArrow } else { style.end };
    let start_kind = if shows_arrow(c, false) && style.start == End::None { End::OpenArrow } else { style.start };
    arrow_end(painter, path[n - 1], end_dir, end_kind, solid, look, z, opts.erd_notation);
    arrow_end(painter, path[0], start_dir, start_kind, solid, look, z, opts.erd_notation);

    if look.icon {
        return [None; 3];
    }
    // labels
    let f = font_id(look, look.text_size * 0.95 * z);
    let tc = look.text();
    let (mid, dir) = if n == 2 {
        (path[0] + (path[1] - path[0]) * 0.5, path[1] - path[0])
    } else {
        let k = n / 2;
        (path[k - 1] + (path[k] - path[k - 1]) * 0.5, path[k] - path[k - 1])
    };
    let mut label = String::new();
    if let Some(k) = style.keyword {
        label.push_str(&format!("«{k}»"));
    }
    let mut name = c.name.clone();
    if dkind == DiagramKind::Sequence && c.kind.is_message() {
        if let Some(num) = number {
            // every message is numbered, return messages too (no number when numbering is off
            // for the diagram or the project: seq::layout leaves it out)
            name = if c.name.is_empty() { num.to_string() } else { format!("{num}: {}", c.name) };
        }
    }
    if !name.is_empty() {
        if !label.is_empty() {
            label.push(' ');
        }
        label.push_str(&name);
    }
    let mut label_rect = None;
    if !label.is_empty() {
        let off = Vec2::new(c.label_offset.0, c.label_offset.1) * z;
        let horizontal = dir.x.abs() >= dir.y.abs();
        let r = if horizontal {
            painter.text(mid - Vec2::new(0.0, 3.0 * z) + off, Align2::CENTER_BOTTOM, &label, f.clone(), tc)
        } else {
            painter.text(mid + Vec2::new(6.0 * z, 0.0) + off, Align2::LEFT_CENTER, &label, f.clone(), tc)
        };
        // a name dragged far from its line keeps a faint leader to it
        if off.length() > 24.0 * z {
            let to = r.center() + (mid - r.center()).normalized() * (r.width().min(r.height()) * 0.5);
            dashed(painter, &[r.center(), mid], Stroke::new(0.6_f32, look.line().linear_multiply(0.45)), 0.5);
            let _ = to;
        }
        label_rect = Some(r.expand(2.0));
    }
    let mut ends = [None, None];
    if opts.show_multiplicity {
        // the source / target labels sit beside their end, or where they were dragged to
        let place = |painter: &Painter, at: Pos2, toward: Vec2, text: &str, off: (f32, f32)| -> Option<Rect> {
            if text.is_empty() {
                return None;
            }
            let d = toward.normalized();
            let nrm = Vec2::new(-d.y, d.x);
            let home = at + d * 16.0 * z + nrm * 10.0 * z;
            let off = Vec2::new(off.0, off.1) * z;
            let r = painter.text(home + off, Align2::CENTER_CENTER, text, f.clone(), tc);
            if off.length() > 24.0 * z {
                dashed(painter, &[r.center(), at], Stroke::new(0.6_f32, look.line().linear_multiply(0.45)), 0.5);
            }
            Some(r.expand(2.0))
        };
        ends[0] = place(painter, path[0], path[1] - path[0], &c.from_label, c.from_label_offset);
        ends[1] = place(painter, path[n - 1], path[n - 2] - path[n - 1], &c.to_label, c.to_label_offset);
    }
    [label_rect, ends[0], ends[1]]
}

/// Radius of the arc a line draws to jump over another line (screen px at 100%).
pub const HOP_RADIUS: f32 = 5.0;

/// Shapes whose name is drawn below them (and can be dragged).
pub fn has_name_below(k: ShapeKind) -> bool {
    use ShapeKind::*;
    matches!(
        k,
        Port | ProvidedInterface | RequiredInterface | Actor | Initial | Final | ActivityFinal | FlowFinal | ExitPoint | EntryPoint | Terminate | Junction | AcceptTimeEvent | Router | Switch | Hub | Server | Firewall | WebFirewall | LoadBalancer | Pc | Laptop | Phone | AccessPoint | Storage | Database | Person
    )
}

/// The screen rectangle of a shape's name drawn below it, if it has one.
pub fn name_below_rect(painter: &Painter, cam: &Cam, shape: &Shape, look: &Look) -> Option<Rect> {
    if !has_name_below(shape.kind) || shape.name.is_empty() {
        return None;
    }
    let look = shape_look(shape, look);
    let r = cam.rect(shape);
    let galley = painter.layout_no_wrap(shape.name.clone(), font(&look, cam, 1.0), Color32::WHITE);
    let off = Vec2::new(shape.label_offset.0, shape.label_offset.1) * cam.zoom;
    let top = Pos2::new(r.center().x, r.bottom() + 3.0 * cam.zoom) + off;
    Some(Rect::from_min_size(Pos2::new(top.x - galley.size().x / 2.0, top.y), galley.size()).expand(2.0))
}

/// The area a diagram covers (diagram units), with room for keywords above shapes, names and
/// labels below and beside them, bend points, and a 12-unit margin. None for an empty diagram.
pub fn diagram_bounds(d: &crate::model::Diagram) -> Option<(Pos2, Pos2)> {
    if d.shapes.is_empty() {
        return None;
    }
    let mut min = Pos2::new(f32::MAX, f32::MAX);
    let mut max = Pos2::new(f32::MIN, f32::MIN);
    let mut take = |x: f32, y: f32| {
        min.x = min.x.min(x);
        min.y = min.y.min(y);
        max.x = max.x.max(x);
        max.y = max.y.max(y);
    };
    for s in &d.shapes {
        take(s.x - 10.0, s.y - 30.0);
        take(s.x + s.w + 30.0, s.y + s.h + 24.0);
    }
    for c in &d.connectors {
        for (x, y) in &c.points {
            take(x - 10.0, y - 10.0);
            take(x + 10.0, y + 10.0);
        }
    }
    Some((min - Vec2::splat(12.0), max + Vec2::splat(12.0)))
}

/// Paint a whole diagram as the canvas shows it, without selection marks or handles:
/// containers (largest first), other shapes, activation bars, lines (with jumps where they
/// cross and message numbers), then network device names over the lines.
pub fn paint_diagram(painter: &Painter, cam: &Cam, d: &crate::model::Diagram, look: &Look, opts: &ProjectOptions) {
    let mut containers: Vec<&Shape> = d.shapes.iter().filter(|s| s.kind.is_container()).collect();
    containers.sort_by(|a, b| (b.w * b.h).partial_cmp(&(a.w * a.h)).unwrap_or(std::cmp::Ordering::Equal));
    for s in containers {
        draw_shape(painter, cam, s, look, opts);
    }
    for s in d.shapes.iter().filter(|s| !s.kind.is_container()) {
        draw_shape(painter, cam, s, look, opts);
    }
    let (lay, paths) = crate::seq::all_paths(d, opts);
    if opts.show_activations {
        for bar in &lay.bars {
            if let Some(l) = d.shape(bar.lifeline) {
                draw_activation(painter, cam, bar, l, look);
            }
        }
    }
    let screen: Vec<(u64, Vec<Pos2>)> = d.connectors.iter().filter_map(|c| paths.get(&c.id).map(|p| (c.id, p.iter().map(|(x, y)| cam.pt(*x, *y)).collect()))).collect();
    let hops = crossings(&screen);
    for c in &d.connectors {
        let Some(path) = paths.get(&c.id) else { continue };
        let Some(a) = d.shape(c.from) else { continue };
        let b = d.shape(c.to).unwrap_or(a);
        let h = hops.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]);
        draw_connector(painter, cam, d.kind, c, a, b, look, opts, lay.numbers.get(&c.id).map(|s| s.as_str()), path, h, false);
    }
    for s in &d.shapes {
        device_name_over_lines(painter, cam, s, look);
    }
}

/// A network device's name drawn again on top of the lines, on a small patch of the
/// background, so a line leaving the device downwards doesn't strike through it.
pub fn device_name_over_lines(painter: &Painter, cam: &Cam, shape: &Shape, look: &Look) {
    if !shape.kind.is_network_device() {
        return;
    }
    let Some(rect) = name_below_rect(painter, cam, shape, look) else { return };
    let l = shape_look(shape, look);
    painter.rect_filled(rect, 2.0, l.background());
    let r = cam.rect(shape);
    let off = Vec2::new(shape.label_offset.0, shape.label_offset.1) * cam.zoom;
    let top = Pos2::new(r.center().x, r.bottom() + 3.0 * cam.zoom) + off;
    text_at(painter, &l, top, Align2::CENTER_TOP, &shape.name, font(&l, cam, 1.0), l.text());
}

/// For every pair of crossing lines, decide which one jumps: the more horizontal segment
/// jumps over the more vertical one (ties: the later line). Returns, per connector id,
/// (segment index, position along the segment 0..1) of each jump.
pub fn crossings(paths: &[(u64, Vec<Pos2>)]) -> std::collections::HashMap<u64, Vec<(usize, f32)>> {
    let mut out: std::collections::HashMap<u64, Vec<(usize, f32)>> = std::collections::HashMap::new();
    for i in 0..paths.len() {
        for j in (i + 1)..paths.len() {
            let (pa, pb) = (&paths[i].1, &paths[j].1);
            for sa in 0..pa.len().saturating_sub(1) {
                for sb in 0..pb.len().saturating_sub(1) {
                    let (a0, a1, b0, b1) = (pa[sa], pa[sa + 1], pb[sb], pb[sb + 1]);
                    let r = a1 - a0;
                    let s = b1 - b0;
                    let den = r.x * s.y - r.y * s.x;
                    if den.abs() < 1e-6 {
                        continue;
                    }
                    let q = b0 - a0;
                    let t = (q.x * s.y - q.y * s.x) / den;
                    let u = (q.x * r.y - q.y * r.x) / den;
                    if t <= 0.0 || t >= 1.0 || u <= 0.0 || u >= 1.0 {
                        continue;
                    }
                    let ha = r.x.abs() / r.length().max(1e-6);
                    let hb = s.x.abs() / s.length().max(1e-6);
                    if ha > hb + 1e-3 {
                        out.entry(paths[i].0).or_default().push((sa, t));
                    } else {
                        out.entry(paths[j].0).or_default().push((sb, u));
                    }
                }
            }
        }
    }
    out
}

/// The polyline with a small half-circle wherever it jumps over another line.
pub fn apply_hops(path: &[Pos2], hops: &[(usize, f32)], radius: f32) -> Vec<Pos2> {
    if hops.is_empty() || path.len() < 2 {
        return path.to_vec();
    }
    let mut out = vec![path[0]];
    for i in 0..path.len() - 1 {
        let (a, b) = (path[i], path[i + 1]);
        let d = b - a;
        let len = d.length();
        if len < 1e-3 {
            out.push(b);
            continue;
        }
        let u = d / len;
        // the arc bulges upward (or to the left for vertical lines)
        let mut n = Vec2::new(-u.y, u.x);
        if n.y > 0.0 || (n.y.abs() < 1e-6 && n.x > 0.0) {
            n = -n;
        }
        let mut ts: Vec<f32> = hops.iter().filter(|h| h.0 == i).map(|h| h.1 * len).filter(|s| *s > radius * 1.5 && *s < len - radius * 1.5).collect();
        ts.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
        let mut last = f32::MIN;
        for s in ts {
            if s - last < radius * 2.2 {
                continue;
            }
            last = s;
            let c = a + u * s;
            for k in 0..=8 {
                let ang = PI * k as f32 / 8.0;
                out.push(c - u * radius * ang.cos() + n * radius * ang.sin());
            }
        }
        out.push(b);
    }
    out
}

/// Does an end of the line show a navigability arrow? An end that is navigable (True)
/// while the other end is not shows one; two navigable ends are drawn without arrows
/// (estimated display rule). Old files' explicit arrows still show.
pub fn shows_arrow(c: &Connector, target: bool) -> bool {
    let legacy = if target { c.arrow_end } else { c.arrow_start };
    if legacy {
        return true;
    }
    if !c.kind.has_navigability() {
        return false;
    }
    let (me, other) = if target { (c.nav_to, c.nav_from) } else { (c.nav_from, c.nav_to) };
    me == crate::model::NAV_TRUE && other != crate::model::NAV_TRUE
}

/// An activation bar on a lifeline (filled like the lifeline, estimated).
pub fn draw_activation(painter: &Painter, cam: &Cam, bar: &crate::seq::Bar, lifeline: &Shape, look: &Look) {
    use crate::seq::{BAR_HALF, BAR_STEP};
    let cx = lifeline.x + lifeline.w / 2.0 + bar.depth as f32 * BAR_STEP;
    let r = Rect::from_min_max(cam.pt(cx - BAR_HALF, bar.y0), cam.pt(cx + BAR_HALF, bar.y1));
    let fill = lifeline.fill.map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])).unwrap_or_else(|| look.fill(ShapeKind::Lifeline));
    painter.rect(r, 0.0, fill, Stroke::new((1.0 * cam.zoom).clamp(0.6, 2.0), look.line()), StrokeKind::Middle);
}

/// Distance from a point to a polyline (screen units).
pub fn distance_to_path(p: Pos2, path: &[Pos2]) -> f32 {
    let mut best = f32::INFINITY;
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let t = if ab.length_sq() < 1e-6 { 0.0 } else { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) };
        let q = a + ab * t;
        best = best.min((p - q).length());
    }
    best
}

/// A small picture of a toolbox entry.
pub fn draw_icon(painter: &Painter, rect: Rect, tool: Tool, look: &Look, opts: &ProjectOptions) {
    let icon_look = Look { icon: true, text_size: 1.0, ..*look };
    match tool {
        Tool::Select => {
            let c = rect.center();
            let s = rect.height() * 0.42;
            let pts = vec![
                Pos2::new(c.x - s * 0.5, c.y - s),
                Pos2::new(c.x + s * 0.55, c.y + s * 0.15),
                Pos2::new(c.x - s * 0.5, c.y + s * 0.45),
            ];
            painter.add(epaint::Shape::convex_polygon(pts, look.text(), Stroke::NONE));
        }
        Tool::Shape(kind) => {
            let (w, h) = match kind {
                ShapeKind::Lifeline | ShapeKind::ActorLifeline => (60.0, 100.0),
                ShapeKind::System | ShapeKind::VerticalSwimlane => (100.0, 120.0),
                ShapeKind::HorizontalSwimlane => (130.0, 80.0),
                ShapeKind::Fork | ShapeKind::Join => (100.0, 14.0),
                ShapeKind::Constraint => (60.0, 40.0),
                ShapeKind::Text => (40.0, 40.0),
                k => k.default_size(),
            };
            let avail = rect.shrink(1.0);
            let zoom = (avail.width() / w).min(avail.height() / h);
            let size = Vec2::new(w, h) * zoom;
            let origin = avail.center() - size / 2.0;
            let cam = Cam { origin, pan: Vec2::ZERO, zoom };
            let mut shape = Shape {
                id: 0,
                kind,
                x: 0.0,
                y: 0.0,
                w,
                h,
                name: String::new(),
                stereotype: String::new(),
                attributes: if kind.has_attributes() && kind != ShapeKind::State { vec![String::new()] } else { vec![] },
                operations: vec![],
                fill: None,
                documentation: String::new(),
                font: None,
                font_size: None,
                orient: 0,
                hide_icon: false,
                hide_attributes: false,
                hide_operations: false,
                splits: Vec::new(),
                act_spans: Vec::new(),
                dividers: None,
                partitions: None,
                label_offset: (0.0, 0.0),
            };
            if kind == ShapeKind::Constraint {
                shape.name = "{}".into();
            }
            // icons keep the frame visible at tiny sizes
            draw_shape(painter, &cam, &shape, &icon_look, opts);
        }
        Tool::Conn(kind) => {
            let a = Pos2::new(rect.left() + 2.0, rect.bottom() - 2.0);
            let b = Pos2::new(rect.right() - 2.0, rect.top() + 2.0);
            let stroke = Stroke::new(1.2_f32, look.line());
            let style = kind.style();
            if kind == ConnKind::Branch {
                // a mind map branch: an S curve
                let n = 12;
                let pts: Vec<Pos2> = (0..=n)
                    .map(|i| {
                        let t = i as f32 / n as f32;
                        let s = t * t * (3.0 - 2.0 * t);
                        Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * s)
                    })
                    .collect();
                painter.add(epaint::Shape::line(pts, stroke));
            } else if style.dashed {
                dashed(painter, &[a, b], stroke, 0.5);
            } else {
                painter.line_segment([a, b], stroke);
            }
            arrow_end(painter, b, b - a, style.end, stroke, &icon_look, 0.55, ErdNotation::CrowsFoot);
            arrow_end(painter, a, a - b, style.start, stroke, &icon_look, 0.55, ErdNotation::CrowsFoot);
        }
    }
}

/// Where the attribute and operation lists sit inside a shape, in diagram units
/// measured from the shape's top. Mirrors the layout used by `draw_shape`.
pub struct Compartments {
    pub lh: f32,
    pub attr: Option<(f32, f32)>,
    pub ops: Option<(f32, f32)>,
}

pub fn compartments(shape: &Shape, opts: &ProjectOptions, look: &Look) -> Option<Compartments> {
    use ShapeKind::*;
    if !matches!(shape.kind, Class | Interface | Enumeration | Instance | Entity | DbView | State) {
        return None;
    }
    let look = shape_look(shape, look);
    let lh = line_height(&look);
    let kw = keyword(shape, opts).is_some();
    let is_entity = matches!(shape.kind, Entity | DbView);
    let header = 8.0 + lh + if kw { lh * 0.9 } else { 0.0 } + if is_entity { 4.0 } else { 0.0 };
    let mut y = header;
    let mut attr = None;
    if visible_attributes(shape, opts) && (shape.kind != State || !shape.attributes.is_empty()) {
        let n = shape.attributes.len().max(1) as f32;
        let y1 = y + 4.0 + n * lh + 4.0;
        attr = Some((y, y1));
        y = y1;
    }
    let mut ops = None;
    if visible_operations(shape, opts) {
        ops = Some((y, shape.h.max(y + lh)));
    } else if let Some((a0, a1)) = attr {
        attr = Some((a0, shape.h.max(a1)));
    }
    Some(Compartments { lh, attr, ops })
}
