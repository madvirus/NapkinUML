//! Zooming, fitting and centring the view.

use super::*;

impl UmlApp {
    /// Zoom In as the original does it (v.ddp.j.bD): +10 points while below 800 %.
    pub(super) fn zoom_in_step(&mut self, around: Option<Pos2>) {
        let z = self.view().zoom;
        if z < ZOOM_MAX {
            self.zoom_set(z + 0.1, around);
        }
    }

    /// Zoom Out as the original does it (v.ddp.j.bE): -10 points above 10 %,
    /// otherwise x0.9, and never at or below 2 %.
    pub(super) fn zoom_out_step(&mut self, around: Option<Pos2>) {
        let z = self.view().zoom;
        let n = if z > 0.1 { z - 0.1 } else { z * 0.9 };
        if n > ZOOM_MIN {
            self.zoom_set(n, around);
        }
    }

    pub(super) fn zoom_set(&mut self, z: f32, around: Option<Pos2>) {
        let f = z / self.view().zoom;
        self.zoom_by(f, around);
    }

    pub(super) fn zoom_by(&mut self, factor: f32, around: Option<Pos2>) {
        let rect = self.canvas_rect;
        let Some(v) = self.view_mut() else { return };
        let old = v.zoom;
        let new = (old * factor).clamp(ZOOM_MIN, ZOOM_MAX);
        let anchor = around.unwrap_or(if rect.is_positive() { rect.center() } else { Pos2::ZERO });
        // keep the diagram point under the anchor fixed
        let rel = anchor - rect.min - v.pan;
        v.pan += rel - rel * (new / old);
        v.zoom = new;
    }

    pub(super) fn zoom_to(&mut self, z: f32) {
        let f = z / self.view().zoom;
        self.zoom_by(f, None);
    }

    pub(super) fn fit_to_window(&mut self) {
        self.fit_view(2.0, false);
    }

    /// Fit the diagram into the canvas, never zooming in past `max_zoom`.
    /// With `only_if_needed`, a diagram that already fits at 100% is left alone.
    pub(super) fn fit_view(&mut self, max_zoom: f32, only_if_needed: bool) {
        let rect = self.canvas_rect;
        let Some(d) = self.diag() else { return };
        if d.shapes.is_empty() || !rect.is_positive() {
            if let Some(v) = self.view_mut() {
                v.zoom = 1.0;
                v.pan = Vec2::new(20.0, 20.0);
            }
            return;
        }
        let mut min = Pos2::new(f32::MAX, f32::MAX);
        let mut max = Pos2::new(f32::MIN, f32::MIN);
        for s in &d.shapes {
            min.x = min.x.min(s.x);
            min.y = min.y.min(s.y - 30.0);
            max.x = max.x.max(s.x + s.w + 30.0);
            max.y = max.y.max(s.y + s.h + 20.0);
        }
        let w = (max.x - min.x).max(1.0);
        let h = (max.y - min.y).max(1.0);
        let z = ((rect.width() - 40.0) / w).min((rect.height() - 40.0) / h).clamp(0.1, max_zoom);
        if only_if_needed && 20.0 + max.x <= rect.width() && 20.0 + max.y <= rect.height() && min.x >= -20.0 && min.y >= -20.0 {
            return;
        }
        if let Some(v) = self.view_mut() {
            v.zoom = z;
            v.pan = if only_if_needed {
                // first look: keep the diagram at the top left, like at 100%
                Vec2::new(20.0 - min.x * z, 20.0 - (min.y + 30.0) * z)
            } else {
                Vec2::new(rect.width() / 2.0 - (min.x + w / 2.0) * z, rect.height() / 2.0 - (min.y + h / 2.0) * z)
            };
        }
    }

    pub(super) fn center_on(&mut self, id: u64) {
        let rect = self.canvas_rect;
        let Some((cx, cy)) = self.diag().and_then(|d| d.shape(id)).map(|s| s.center()) else { return };
        if let Some(v) = self.view_mut() {
            if rect.is_positive() {
                v.pan = Vec2::new(rect.width() / 2.0 - cx * v.zoom, rect.height() / 2.0 - cy * v.zoom);
            }
        }
    }
}
