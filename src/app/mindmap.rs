//! Mind map editing: Tab adds a sub-topic to the selected topic, Enter a sibling topic, and a
//! dragged topic joins the topic it is dropped next to. Placement is mindlayout's.

use super::*;
use crate::model::mind_parent;

impl UmlApp {
    /// The mind map topic the Tab and Enter keys work on: the topic whose name is being typed,
    /// or the one selected topic while nothing else has the keyboard.
    pub(super) fn mind_target(&self, ctx: &egui::Context) -> Option<u64> {
        let d = self.diag()?;
        if d.kind != DiagramKind::MindMap {
            return None;
        }
        let dialog_open = self.switcher.is_some() || self.jobs.any_open() || self.confirm.is_some() || self.confirm_delete.is_some() || self.new_diagram.is_some() || self.app_opts.is_some() || self.proj_opts.is_some() || self.rename.is_some();
        if dialog_open {
            return None;
        }
        if let Some(ed) = &self.editing {
            return match (ed.target, ed.line, ed.part, ed.guard) {
                (EditTarget::Shape(id), None, None, None) => d.shape(id).filter(|s| s.kind.is_topic()).map(|s| s.id),
                _ => None,
            };
        }
        if ctx.wants_keyboard_input() || self.tree_focus || self.sel_shapes.len() != 1 || !self.sel_conns.is_empty() {
            return None;
        }
        d.shape(self.sel_shapes[0]).filter(|s| s.kind.is_topic()).map(|s| s.id)
    }

    /// Called before egui reads the input: egui would move the keyboard focus with Tab, so a
    /// Tab meant for a mind map topic is taken out here and handled in `shortcuts`.
    pub(super) fn mind_tab(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let is_tab = |e: &egui::Event| matches!(e, egui::Event::Key { key: Key::Tab, modifiers, .. } if modifiers.is_none());
        if !raw.events.iter().any(is_tab) {
            return;
        }
        let Some(id) = self.mind_target(ctx) else { return };
        let pressed = raw.events.iter().any(|e| matches!(e, egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if modifiers.is_none()));
        raw.events.retain(|e| !is_tab(e));
        if pressed {
            self.mind_pending = Some(id);
            ctx.request_repaint();
        }
    }

    /// Add a topic next to topic `id`: a sub-topic of it (`child`), or a sibling under the same
    /// parent (a sub-topic when it has no parent, as the central topic). The new topic is put
    /// where it belongs in its parent's order (a sub-topic last; a sibling right after the
    /// selected topic; for a mind map root both in clockwise order: down the right side while
    /// the left is empty, then up the left side), joined by a Branch, and the structure places
    /// it (evening out the sides only past `REBALANCE_AFTER`). One undo step; selected with its name ready to type over.
    pub(super) fn mind_add(&mut self, id: u64, child: bool) {
        use crate::mindlayout::{self as ml, Grow};
        if child {
            // a folded topic opens to show its new sub-topic among the others
            self.mind_set_fold(id, false);
        }
        let Some(d) = self.diag() else { return };
        if d.kind != DiagramKind::MindMap {
            return;
        }
        let Some(sel) = d.shape(id).cloned() else { return };
        let parent_id = if child { id } else { mind_parent(d, id).unwrap_or(id) };
        let child = child || parent_id == id;
        let Some(parent) = d.shape(parent_id).cloned() else { return };
        let (pcx, pcy) = parent.center();
        let grow = ml::grow(d, parent_id).unwrap_or(Grow::Right);
        let kids: Vec<Shape> = ml::children(d, parent_id).into_iter().filter_map(|k| d.shape(k).cloned()).collect();
        let (w, h) = ShapeKind::Topic.default_size();
        let right = match grow {
            Grow::Right | Grow::Down => true,
            Grow::Left => false,
            Grow::Both if !child => sel.center().0 >= pcx,
            // clockwise: down the right side while the left is empty, then up the left side
            Grow::Both => !kids.iter().any(|k| k.center().0 < pcx),
        };
        // on a mind map root's left side the clockwise order runs upward
        let upward = grow == Grow::Both && !right;
        // the new topic's centre: its place in the order the structure keeps
        let (cx, cy) = if grow == Grow::Down {
            let x = if child { kids.iter().map(|k| k.center().0).fold(pcx, f32::max) + 1.0 } else { sel.center().0 + 0.5 };
            (x, parent.y + parent.h + ml::GAP_V + h / 2.0)
        } else {
            let side: Vec<f32> = kids.iter().filter(|k| (k.center().0 >= pcx) == right).map(|k| k.center().1).collect();
            let y = if !child {
                sel.center().1 + if upward { -0.5 } else { 0.5 }
            } else if side.is_empty() {
                pcy
            } else if upward {
                side.iter().copied().fold(f32::INFINITY, f32::min) - 1.0
            } else {
                side.iter().copied().fold(f32::NEG_INFINITY, f32::max) + 1.0
            };
            let x = if right { parent.x + parent.w + ml::GAP_X + w / 2.0 } else { parent.x - ml::GAP_X - w / 2.0 };
            (x, y)
        };

        let undo_before = self.undo.len();
        let keep_tool = self.tool;
        self.create_shape(ShapeKind::Topic, cx, cy);
        self.tool = keep_tool;
        let Some(&new_id) = self.sel_shapes.first() else { return };
        if new_id == parent_id {
            return;
        }
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(new_id)) {
            s.x = cx - s.w / 2.0;
            s.y = cy - s.h / 2.0;
        }
        let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
        self.create_connector(ConnKind::Branch, parent_id, new_id, 0.0);
        if self.diag().map(|dm| dm.connectors.len()).unwrap_or(0) > before && self.undo.len() > undo_before + 1 {
            // the topic and its branch undo together
            self.undo.pop();
        }
        if let Some(dm) = self.diag_mut() {
            ml::layout(dm);
        }
        self.tool = Tool::Select;
        self.clear_selection();
        self.sel_shapes.push(new_id);
        if let Some(ed) = self.editing.as_mut() {
            // "Topic3" is a placeholder: typing replaces it
            ed.select_all = true;
        }
        self.keep_in_view(new_id);
    }

    /// The topic being dragged on its own in a mind map (with its sub-topics), if any.
    pub(super) fn mind_dragged(&self, d: &Diagram) -> Option<u64> {
        if d.kind != DiagramKind::MindMap || self.sel_shapes.len() != 1 {
            return None;
        }
        d.shape(self.sel_shapes[0]).filter(|s| s.kind == ShapeKind::Topic).map(|s| s.id)
    }

    /// A topic dropped after a drag joins the topic it landed next to (see
    /// `mindlayout::drop_parent`), keeping its sub-topics, or floats free when it landed far from
    /// every topic; then the structure places everything again. Part of the move's undo step.
    pub(super) fn mind_drop(&mut self) {
        let Some(d) = self.diag() else { return };
        let Some(id) = self.mind_dragged(d) else {
            if let Some(dm) = self.diag_mut() {
                crate::mindlayout::layout(dm);
            }
            return;
        };
        let target = crate::mindlayout::drop_parent(d, id).map(|t| t.0);
        if let Some(t) = target {
            // dropped next to a folded topic: it opens to take the new sub-topic
            if self.mind_set_fold(t, false) {
                crate::mindlayout::layout(self.diag_mut().unwrap());
            }
        }
        let Some(d) = self.diag() else { return };
        let old = d.connectors.iter().position(|c| c.kind == ConnKind::Branch && c.to == id && c.from != id && d.shape(c.from).map(|s| s.kind.is_topic()).unwrap_or(false));
        let fresh = self.project.as_mut().map(|p| p.fresh_id());
        let Some(dm) = self.diag_mut() else { return };
        match (target, old) {
            (Some(p), Some(i)) => dm.connectors[i].from = p,
            (Some(p), None) => {
                if let Some(cid) = fresh {
                    dm.connectors.push(Connector {
                        id: cid,
                        kind: ConnKind::Branch,
                        from: p,
                        to: id,
                        name: String::new(),
                        from_label: String::new(),
                        to_label: String::new(),
                        y: 0.0,
                        documentation: String::new(),
                        points: vec![],
                        arrow_start: false,
                        arrow_end: false,
                        style: render::STYLE_FOLLOW_DIAGRAM,
                        nav_from: crate::model::NAV_TRUE,
                        nav_to: crate::model::NAV_TRUE,
                        seq_no: String::new(),
                        from_label_offset: (0.0, 0.0),
                        to_label_offset: (0.0, 0.0),
                        label_offset: (0.0, 0.0),
                        from_anchor: None,
                        to_anchor: None,
                    });
                }
            }
            (None, Some(i)) => {
                dm.connectors.remove(i);
            }
            (None, None) => {}
        }
        crate::mindlayout::layout_keep(dm, Some(id));
    }

    /// The topic whose fold knob is under a screen point (see `render::fold_knob`).
    pub(super) fn mind_knob_at(d: &Diagram, cam: &Cam, p: Pos2) -> Option<u64> {
        crate::mindlayout::fold_knobs(d)
            .into_iter()
            .find(|(id, g, _)| {
                d.shape(*id)
                    .map(|s| {
                        let (c, r) = render::fold_knob(cam, s, *g);
                        (p - c).length() <= r + 2.0
                    })
                    .unwrap_or(false)
            })
            .map(|k| k.0)
    }

    /// Fold (`fold`) or unfold a topic of the diagram shown, without an undo step or layout.
    /// True when it changed.
    fn mind_set_fold(&mut self, id: u64, fold: bool) -> bool {
        let Some(active) = self.active else { return false };
        let Some(p) = self.project.as_mut() else { return false };
        let mut next = p.next_id;
        let Some(d) = p.diagram_mut(active) else { return false };
        let changed = if fold {
            crate::model::mind_fold(d, id)
        } else {
            crate::model::mind_unfold(d, id, || {
                next += 1;
                next - 1
            })
        };
        p.next_id = next;
        changed
    }

    /// Fold a topic's sub-topics away, or unfold a folded topic: one undo step, then the
    /// structure places everything again. Folded shapes and lines leave the selection.
    pub(super) fn mind_toggle(&mut self, id: u64) {
        let Some(d) = self.diag() else { return };
        if !crate::model::mind_foldable(d, id) {
            return;
        }
        let folded = d.shape(id).map(|s| s.fold.is_some()).unwrap_or(false);
        self.commit_edit();
        let undo_before = self.undo.len();
        self.checkpoint();
        if !self.mind_set_fold(id, !folded) {
            self.undo.truncate(undo_before);
            return;
        }
        let Some(dm) = self.diag_mut() else { return };
        crate::mindlayout::layout(dm);
        let shapes: std::collections::HashSet<u64> = dm.shapes.iter().map(|s| s.id).collect();
        let conns: std::collections::HashSet<u64> = dm.connectors.iter().map(|c| c.id).collect();
        self.sel_shapes.retain(|x| shapes.contains(x));
        self.sel_conns.retain(|x| conns.contains(x));
    }

    /// While a topic is dragged: outline the topic it would join and draw a dashed line from
    /// there to it.
    pub(super) fn mind_drop_preview(&self, painter: &egui::Painter, cam: &Cam, d: &Diagram, look: &Look) {
        let Some(id) = self.mind_dragged(d) else { return };
        let Some((p, g)) = crate::mindlayout::drop_parent(d, id) else { return };
        let (Some(ps), Some(me)) = (d.shape(p), d.shape(id)) else { return };
        let (pr, mr) = (cam.rect(ps), cam.rect(me));
        let st = Stroke::new(2.0_f32, look.accent());
        painter.rect_stroke(pr.expand(4.0), 6.0, st, StrokeKind::Middle);
        let (a, b) = match g {
            crate::mindlayout::Grow::Left => (pr.left_center(), mr.right_center()),
            crate::mindlayout::Grow::Down => (pr.center_bottom(), mr.center_top()),
            _ => (pr.right_center(), mr.left_center()),
        };
        let n = 12;
        for i in (0..n).step_by(2) {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            painter.line_segment([a + (b - a) * t0, a + (b - a) * t1], st);
        }
    }

    /// An arrow key on a selected topic: select the topic that way (`mindlayout::neighbour`)
    /// and scroll it into view. Nothing happens when there is none.
    pub(super) fn mind_step(&mut self, id: u64, dir: (i8, i8)) {
        let Some(next) = self.diag().and_then(|d| crate::mindlayout::neighbour(d, id, dir)) else { return };
        self.clear_selection();
        self.sel_shapes.push(next);
        self.keep_in_view(next);
    }

    /// Scroll the canvas just enough that the shape is in view (with a small margin).
    fn keep_in_view(&mut self, id: u64) {
        let rect = self.canvas_rect;
        let Some((x, y, w, h)) = self.diag().and_then(|d| d.shape(id)).map(|s| (s.x, s.y, s.w, s.h)) else { return };
        let Some(v) = self.view_mut() else { return };
        if !rect.is_positive() || v.zoom <= 0.0 {
            return;
        }
        const M: f32 = 30.0;
        let z = v.zoom;
        // the shape's screen box relative to the canvas
        let (l, t) = (x * z + v.pan.x, y * z + v.pan.y);
        let (r, b) = (l + w * z, t + h * z);
        if l < M {
            v.pan.x += M - l;
        } else if r > rect.width() - M {
            v.pan.x -= (r - (rect.width() - M)).min(l - M);
        }
        if t < M {
            v.pan.y += M - t;
        } else if b > rect.height() - M {
            v.pan.y -= (b - (rect.height() - M)).min(t - M);
        }
    }
}
