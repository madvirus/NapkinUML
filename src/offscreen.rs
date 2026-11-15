//! Drawing a diagram into a picture without showing it on screen (for printing and exporting).
//!
//! A separate, hidden egui context lays out and paints the diagram with the same drawing code
//! as the canvas; its triangles are then filled in by a small software rasterizer (colours and
//! font textures, alpha blending as egui does), so nothing in the window changes.

use crate::model::{Diagram, ProjectOptions};
use crate::render::{self, Cam, Look};
use egui::epaint::{ClippedPrimitive, Color32, ImageData, Primitive, TextureId};
use egui::{Pos2, Rect, Vec2};
use std::collections::HashMap;

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

fn apply_textures(store: &mut HashMap<TextureId, Texture>, delta: egui::TexturesDelta) {
    for (id, d) in delta.set {
        let (w, h, px): (usize, usize, Vec<Color32>) = match &d.image {
            ImageData::Color(img) => (img.size[0], img.size[1], img.pixels.clone()),
            ImageData::Font(img) => (img.size[0], img.size[1], img.srgba_pixels(None).collect()),
        };
        match d.pos {
            None => {
                store.insert(id, Texture { w, h, px });
            }
            Some([x0, y0]) => {
                if let Some(t) = store.get_mut(&id) {
                    for y in 0..h {
                        for x in 0..w {
                            let (tx, ty) = (x0 + x, y0 + y);
                            if tx < t.w && ty < t.h {
                                t.px[ty * t.w + tx] = px[y * w + x];
                            }
                        }
                    }
                }
            }
        }
    }
    for id in delta.free {
        store.remove(&id);
    }
}

fn sample(t: Option<&Texture>, u: f32, v: f32) -> [f32; 4] {
    let Some(t) = t else { return [255.0; 4] };
    if t.w == 0 || t.h == 0 {
        return [255.0; 4];
    }
    // bilinear, like the GPU's linear filtering
    let x = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
    let y = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let p = |xx: usize, yy: usize| t.px[yy * t.w + xx].to_array();
    let (a, b, c, d) = (p(x0, y0), p(x1, y0), p(x0, y1), p(x1, y1));
    let mut out = [0.0; 4];
    for i in 0..4 {
        let top = a[i] as f32 * (1.0 - fx) + b[i] as f32 * fx;
        let bot = c[i] as f32 * (1.0 - fx) + d[i] as f32 * fx;
        out[i] = top * (1.0 - fy) + bot * fy;
    }
    out
}

/// Fill egui's triangles into an RGB picture (premultiplied alpha, blended over what is there).
fn rasterize(prims: &[ClippedPrimitive], textures: &HashMap<TextureId, Texture>, ppp: f32, w: usize, h: usize, bg: Color32) -> image::RgbImage {
    let bgc = bg.to_array();
    let mut buf: Vec<[f32; 3]> = vec![[bgc[0] as f32, bgc[1] as f32, bgc[2] as f32]; w * h];
    for prim in prims {
        let Primitive::Mesh(mesh) = &prim.primitive else { continue };
        let tex = textures.get(&mesh.texture_id);
        let clip = prim.clip_rect;
        let (cx0, cy0) = (((clip.min.x * ppp).floor().max(0.0)) as i64, ((clip.min.y * ppp).floor().max(0.0)) as i64);
        let (cx1, cy1) = (((clip.max.x * ppp).ceil().min(w as f32)) as i64, ((clip.max.y * ppp).ceil().min(h as f32)) as i64);
        for tri in mesh.indices.chunks_exact(3) {
            let v = [&mesh.vertices[tri[0] as usize], &mesh.vertices[tri[1] as usize], &mesh.vertices[tri[2] as usize]];
            let p: Vec<(f32, f32)> = v.iter().map(|q| (q.pos.x * ppp, q.pos.y * ppp)).collect();
            let area = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
            if area.abs() < 1e-6 {
                continue;
            }
            let minx = (p.iter().map(|q| q.0).fold(f32::MAX, f32::min).floor() as i64).max(cx0);
            let maxx = (p.iter().map(|q| q.0).fold(f32::MIN, f32::max).ceil() as i64).min(cx1);
            let miny = (p.iter().map(|q| q.1).fold(f32::MAX, f32::min).floor() as i64).max(cy0);
            let maxy = (p.iter().map(|q| q.1).fold(f32::MIN, f32::max).ceil() as i64).min(cy1);
            if minx >= maxx || miny >= maxy {
                continue;
            }
            let col: Vec<[f32; 4]> = v.iter().map(|q| {
                let c = q.color.to_array();
                [c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32]
            }).collect();
            let solid_white_tex = tex.is_none();
            for y in miny..maxy {
                let py = y as f32 + 0.5;
                for x in minx..maxx {
                    let px = x as f32 + 0.5;
                    let w0 = ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
                    let w1 = ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                        continue;
                    }
                    let mut c = [0.0f32; 4];
                    for i in 0..4 {
                        c[i] = col[0][i] * w0 + col[1][i] * w1 + col[2][i] * w2;
                    }
                    if !solid_white_tex {
                        let u = v[0].uv.x * w0 + v[1].uv.x * w1 + v[2].uv.x * w2;
                        let vv = v[0].uv.y * w0 + v[1].uv.y * w1 + v[2].uv.y * w2;
                        let t = sample(tex, u, vv);
                        for i in 0..4 {
                            c[i] = c[i] * t[i] / 255.0;
                        }
                    }
                    if c[3] <= 0.0 {
                        continue;
                    }
                    let k = 1.0 - c[3] / 255.0;
                    let d = &mut buf[y as usize * w + x as usize];
                    for i in 0..3 {
                        d[i] = c[i] + d[i] * k;
                    }
                }
            }
        }
    }
    let mut out = image::RgbImage::new(w as u32, h as u32);
    for (i, p) in out.pixels_mut().enumerate() {
        let d = buf[i];
        *p = image::Rgb([d[0].round().clamp(0.0, 255.0) as u8, d[1].round().clamp(0.0, 255.0) as u8, d[2].round().clamp(0.0, 255.0) as u8]);
    }
    out
}

fn raw_input(size: Vec2, scale: f32) -> egui::RawInput {
    let mut raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), max_texture_side: Some(8192), ..Default::default() };
    raw.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
    raw
}

/// The picture of a diagram, `scale` pixels per diagram unit (2 = twice the size it has at 100%),
/// never more than `max_px` pixels on its longer side. Shapes are first grown to fit their text
/// (as the canvas does when "Grow shapes to fit their text" is on).
pub fn diagram_picture(d: &Diagram, opts: &ProjectOptions, look: &Look, fonts: &egui::FontDefinitions, scale: f32, max_px: f32) -> image::RgbImage {
    let ctx = egui::Context::default();
    ctx.set_fonts(fonts.clone());
    let mut textures: HashMap<TextureId, Texture> = HashMap::new();
    // pass 1: the fonts become usable; pass 2: grow shapes and measure the diagram
    let out = ctx.run(raw_input(Vec2::new(64.0, 64.0), 1.0), |_| {});
    apply_textures(&mut textures, out.textures_delta);
    let mut d = d.clone();
    let mut bounds = (Pos2::ZERO, Pos2::new(400.0, 300.0));
    let out = ctx.run(raw_input(Vec2::new(64.0, 64.0), 1.0), |ctx| {
        if opts.auto_fit_shapes {
            for s in d.shapes.iter_mut() {
                let m = render::min_size(ctx, s, opts, look);
                s.w = s.w.max(m.x);
                s.h = s.h.max(m.y);
            }
        }
        if let Some(b) = render::diagram_bounds(&d) {
            bounds = b;
        }
    });
    apply_textures(&mut textures, out.textures_delta);
    let size = bounds.1 - bounds.0;
    let scale = scale.min(max_px / size.x.max(size.y)).max(0.1);
    // pass 3: paint, then fill the triangles in
    let out = ctx.run(raw_input(size, scale), |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(Rect::from_min_size(Pos2::ZERO, size), 0.0, look.background());
        let cam = Cam { origin: Pos2::ZERO, pan: -bounds.0.to_vec2(), zoom: 1.0 };
        render::paint_diagram(&painter, &cam, &d, look, opts);
    });
    let ppp = out.pixels_per_point;
    apply_textures(&mut textures, out.textures_delta);
    let prims = ctx.tessellate(out.shapes, ppp);
    let (w, h) = ((size.x * ppp).ceil().max(1.0) as usize, (size.y * ppp).ceil().max(1.0) as usize);
    rasterize(&prims, &textures, ppp, w, h, look.background())
}
