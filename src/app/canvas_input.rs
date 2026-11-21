//! Mouse and keyboard input on the canvas.

use super::*;

impl UmlApp {
    pub(super) fn canvas_input(&mut self, ui: &mut egui::Ui, resp: &egui::Response, cam: &Cam, d: &Diagram) {
        if self.screenshot.is_some() {
            // a check run must not react to the person's mouse
            return;
        }
        let ctx = ui.ctx().clone();
        let pointer = ctx.input(|i| i.pointer.interact_pos()).or(resp.hover_pos());
        let shift = ctx.input(|i| i.modifiers.shift || i.modifiers.command);

        // zoom and scroll
        if resp.hovered() {
            let zd = ctx.input(|i| i.zoom_delta());
            if (zd - 1.0).abs() > 1e-4 {
                // one wheel notch = one Zoom In / Zoom Out step
                if zd > 1.0 {
                    self.zoom_in_step(pointer);
                } else {
                    self.zoom_out_step(pointer);
                }
            } else {
                let sd = ctx.input(|i| i.smooth_scroll_delta);
                if sd != Vec2::ZERO {
                    if let Some(v) = self.view_mut() {
                        v.pan += sd;
                    }
                }
            }
        }
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
            let delta = resp.drag_delta();
            if let Some(v) = self.view_mut() {
                v.pan += delta;
            }
        }

        // primary drags (see drag.rs)
        if resp.drag_started_by(PointerButton::Primary) {
            self.drag_start(&ctx, cam, d, shift);
        }
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = pointer {
                self.drag_update(cam, d, p);
            }
        }
        if resp.drag_stopped_by(PointerButton::Primary) {
            self.drag_finish(cam, d);
        }

        // clicks
        if resp.clicked_by(PointerButton::Primary) {
            if let Some(p) = pointer {
                // a click on an activation bar selects the bar
                let bar_hit = self.tool == Tool::Select && !shift && self.bar_click(d, cam, p);
                match self.tool {
                    Tool::Shape(kind) => {
                        self.commit_edit();
                        let (x, y) = cam.to_diagram(p);
                        self.create_shape(kind, x, y);
                    }
                    Tool::Conn(kind) => {
                        // clicking a lifeline with Message or Asynchronous Message draws a self message there
                        if d.kind == DiagramKind::Sequence && matches!(kind, ConnKind::Message | ConnKind::AsyncMessage) {
                            let hit = self.hit_shape(d, cam, p, true).filter(|id| d.shape(*id).map(|s| s.kind.is_lifeline()).unwrap_or(false));
                            if let Some(id) = hit {
                                self.commit_edit();
                                self.create_connector(kind, id, id, cam.to_diagram(p).1);
                            }
                        }
                    }
                    Tool::Select if bar_hit => {}
                    Tool::Select => match self.label_at(p).map(|t| t.owner()).or_else(|| self.hit_any(cam, p)) {
                        Some(EditTarget::Shape(id)) => {
                            if shift {
                                if let Some(i) = self.sel_shapes.iter().position(|x| *x == id) {
                                    self.sel_shapes.remove(i);
                                } else {
                                    self.sel_shapes.push(id);
                                }
                            } else {
                                self.clear_selection();
                                self.sel_shapes.push(id);
                            }
                        }
                        Some(EditTarget::Conn(id)) => {
                            if !shift {
                                self.clear_selection();
                            }
                            self.sel_conns.push(id);
                        }
                        None => {
                            self.clear_selection();
                        }
                    },
                }
            }
        }
        if resp.double_clicked_by(PointerButton::Primary) && self.tool == Tool::Select {
            if let Some(p) = pointer {
                self.double_click(cam, d, p);
            }
        }
        if resp.secondary_clicked() {
            self.seq_menu = None;
            self.alt_menu = None;
            self.lane_menu = None;
            if let Some(p) = pointer {
                let (x, y) = cam.to_diagram(p);
                if !Self::over_plain_shape(&d, cam, p) {
                    self.lane_menu = d.shapes.iter().rev().find(|s| s.kind.is_swimlane() && x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h).map(|s| (s.id, if s.kind == ShapeKind::VerticalSwimlane { x - s.x } else { y - s.y }));
                }
                self.alt_menu = d.shapes.iter().rev().find(|s| s.kind == ShapeKind::AltFragment && x >= s.x && x <= s.x + s.w && y >= s.y && y <= s.y + s.h).map(|s| (s.id, y - s.y));
            }
            if let Some(p) = pointer {
                // a lifeline's activation bar or split point under the pointer (sequence diagrams)
                let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                self.seq_menu = seq_menu_target(&d, &opts, cam.zoom, cam.to_diagram(p));
                match self.hit_any(cam, p) {
                    Some(EditTarget::Shape(id)) => {
                        if !self.sel_shapes.contains(&id) {
                            self.clear_selection();
                            self.sel_shapes.push(id);
                        }
                    }
                    Some(EditTarget::Conn(id)) => {
                        if !self.sel_conns.contains(&id) {
                            self.clear_selection();
                            self.sel_conns.push(id);
                        }
                    }
                    None => self.clear_selection(),
                }
            }
            self.tool = Tool::Select;
        }
        self.canvas_cursor(&ctx, resp, cam, d, pointer);
        resp.context_menu(|ui| self.canvas_menu(ui));
    }
}
