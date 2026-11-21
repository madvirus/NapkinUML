//! Editing names and member lines in place on the canvas.

use super::*;

pub(super) struct InlineEdit {
    pub(super) target: EditTarget,
    pub(super) text: String,
    pub(super) focus: bool,
    /// Editing one attribute (false) or operation (true) line of a shape, by index.
    pub(super) line: Option<(bool, usize)>,
    /// The line was added for this edit (removed again if left empty).
    pub(super) new_line: bool,
    /// Editing the name of one partition of a swimlane, by index.
    pub(super) part: Option<usize>,
}

/// A name label on the canvas that can be dragged.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum LabelTarget {
    Conn(u64),
    Shape(u64),
    /// The source end label of a line.
    ConnFrom(u64),
    /// The target end label of a line.
    ConnTo(u64),
}

impl LabelTarget {
    pub(super) fn owner(self) -> EditTarget {
        match self {
            LabelTarget::Conn(id) | LabelTarget::ConnFrom(id) | LabelTarget::ConnTo(id) => EditTarget::Conn(id),
            LabelTarget::Shape(id) => EditTarget::Shape(id),
        }
    }
}

impl UmlApp {
    pub(super) fn start_edit(&mut self, target: Option<EditTarget>) {
        let Some(t) = target else { return };
        let Some(d) = self.diag() else { return };
        let text = match t {
            EditTarget::Shape(id) => d.shape(id).map(|s| s.name.clone()),
            EditTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone()),
        };
        if let Some(text) = text {
            self.editing = Some(InlineEdit { target: t, text, focus: true, line: None, new_line: false, part: None });
        }
    }

    /// Which attribute (false) or operation (true) row of a shape is under a screen point.
    /// The index is None when the point is in the list's empty space (a new row).
    pub(super) fn line_at(&self, s: &Shape, cam: &Cam, p: Pos2) -> Option<(bool, Option<usize>)> {
        let opts = self.project.as_ref()?.options.clone();
        let c = render::compartments(s, &opts, &self.look())?;
        let ly = (p.y - cam.pt(0.0, s.y).y) / cam.zoom;
        let row = |y0: f32, len: usize| -> Option<usize> {
            let i = ((ly - y0 - 4.0) / c.lh).floor();
            if i >= 0.0 && (i as usize) < len {
                Some(i as usize)
            } else {
                None
            }
        };
        if let Some((a0, a1)) = c.attr {
            if ly >= a0 && ly < a1 {
                return Some((false, row(a0, s.attributes.len())));
            }
        }
        if let Some((o0, o1)) = c.ops {
            if ly >= o0 && ly <= o1 {
                return Some((true, row(o0, s.operations.len())));
            }
        }
        None
    }

    /// Edit one attribute (ops = false) or operation row in place; None adds a new row.
    pub(super) fn start_line_edit(&mut self, id: u64, ops: bool, idx: Option<usize>) {
        let Some(s) = self.diag().and_then(|d| d.shape(id)).cloned() else { return };
        let list = if ops { &s.operations } else { &s.attributes };
        let (idx, text, new_line) = match idx {
            Some(i) if i < list.len() => (i, list[i].clone(), false),
            _ => {
                let n = list.len();
                self.checkpoint();
                if let Some(t) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                    if ops {
                        t.operations.push(String::new());
                    } else {
                        t.attributes.push(String::new());
                    }
                }
                (n, String::new(), true)
            }
        };
        self.clear_selection();
        self.sel_shapes.push(id);
        self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text, focus: true, line: Some((ops, idx)), new_line, part: None });
    }

    pub(super) fn double_click(&mut self, cam: &Cam, d: &Diagram, p: Pos2) {
        // an end of the selected line: let it be placed automatically again
        if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                if let Some(ends) = self.end_handles(d, cam, c) {
                    if let Some(k) = ends.iter().position(|e| (*e - p).length() < 8.0) {
                        let cid = c.id;
                        self.checkpoint();
                        if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                            if k == 0 {
                                cm.from_anchor = None;
                            } else {
                                cm.to_anchor = None;
                            }
                        }
                        return;
                    }
                }
            }
        }
        // a bend point of the selected line: remove it
        if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
            if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                if let Some(PointHit::Point(i)) = Self::point_handle_hit(d, cam, c, p) {
                    let cid = c.id;
                    self.checkpoint();
                    if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                        if i < cm.points.len() {
                            cm.points.remove(i);
                        }
                    }
                    return;
                }
            }
        }
        // a partition's header: rename that partition
        if self.label_at(p).is_none() && !Self::over_plain_shape(d, cam, p) {
            let (x, y) = cam.to_diagram(p);
            for s in d.shapes.iter().rev().filter(|s| s.kind.is_swimlane()) {
                let parts = s.lane_partitions();
                if let Some(i) = (0..parts.len()).find(|&i| s.partition_header(i).map(|(hx, hy, hw, hh)| x >= hx && x <= hx + hw && y >= hy && y <= hy + hh).unwrap_or(false)) {
                    self.clear_selection();
                    self.sel_shapes.push(s.id);
                    self.editing = Some(InlineEdit { target: EditTarget::Shape(s.id), text: parts[i].name.clone(), focus: true, line: None, new_line: false, part: Some(i) });
                    return;
                }
            }
        }
        let target = self.label_at(p).map(|t| t.owner()).or_else(|| self.hit_any(cam, p));
        if let Some(EditTarget::Shape(id)) = target {
            if let Some(s) = d.shape(id) {
                if let Some((ops, idx)) = self.line_at(s, cam, p) {
                    self.start_line_edit(id, ops, idx);
                    return;
                }
            }
        }
        self.start_edit(target);
    }

    /// Apply the inline editor's text, if one is open.
    pub(super) fn commit_edit(&mut self) {
        let Some(ed) = self.editing.take() else { return };
        if let (EditTarget::Shape(id), Some((ops, idx))) = (ed.target, ed.line) {
            let text = ed.text.trim().to_string();
            let current = self.diag().and_then(|d| d.shape(id)).and_then(|s| if ops { s.operations.get(idx) } else { s.attributes.get(idx) }).cloned();
            let Some(current) = current else { return };
            if current == text && !text.is_empty() {
                return;
            }
            if !ed.new_line {
                self.checkpoint();
            }
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                let list = if ops { &mut s.operations } else { &mut s.attributes };
                if text.is_empty() {
                    if idx < list.len() {
                        list.remove(idx);
                    }
                } else if idx < list.len() {
                    list[idx] = text.clone();
                }
            }
            if ed.new_line && text.is_empty() {
                self.undo.pop();
            }
            return;
        }
        if let (EditTarget::Shape(id), Some(i)) = (ed.target, ed.part) {
            let current = self.diag().and_then(|d| d.shape(id)).and_then(|s| s.lane_partitions().get(i).map(|p| p.name.clone()));
            if current.is_none() || current.as_deref() == Some(ed.text.as_str()) {
                return;
            }
            self.checkpoint();
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                s.normalize_partitions();
                if let Some(p) = s.partitions.as_mut().and_then(|v| v.get_mut(i)) {
                    p.name = ed.text;
                }
            }
            return;
        }
        let Some(d) = self.diag() else { return };
        let current = match ed.target {
            EditTarget::Shape(id) => d.shape(id).map(|s| s.name.clone()),
            EditTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone()),
        };
        if current.is_none() || current.as_deref() == Some(ed.text.as_str()) {
            return;
        }
        self.checkpoint();
        if let Some(dm) = self.diag_mut() {
            match ed.target {
                EditTarget::Shape(id) => {
                    if let Some(s) = dm.shape_mut(id) {
                        s.name = ed.text;
                    }
                }
                EditTarget::Conn(id) => {
                    if let Some(c) = dm.conn_mut(id) {
                        c.name = ed.text;
                    }
                }
            }
        }
    }

    /// Close the inline editor without applying it (a row added for it is removed).
    pub(super) fn cancel_edit(&mut self) {
        let Some(ed) = self.editing.take() else { return };
        if let (EditTarget::Shape(id), Some((ops, idx)), true) = (ed.target, ed.line, ed.new_line) {
            if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) {
                let list = if ops { &mut s.operations } else { &mut s.attributes };
                if idx < list.len() && list[idx].is_empty() {
                    list.remove(idx);
                }
            }
            self.undo.pop();
        }
    }

    pub(super) fn inline_editor(&mut self, ctx: &egui::Context, cam: &Cam) {
        let Some(ed) = self.editing.as_ref() else { return };
        let target = ed.target;
        let line = ed.line;
        let part = ed.part;
        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
        let look = self.look();
        let ds = opts.connector_style;
        let Some(d) = self.diag() else {
            self.editing = None;
            return;
        };
        // (position, width, multiline, left aligned)
        let part_place = match (target, part) {
            (EditTarget::Shape(id), Some(i)) => Some(d.shape(id).and_then(|s| {
                let (x, y, w, _) = s.partition_header(i)?;
                let a = cam.pt(x, y);
                let sw = w * cam.zoom;
                let ww = sw.clamp(100.0, 300.0);
                Some((Pos2::new(a.x + sw / 2.0 - ww / 2.0, a.y + 2.0), ww, false, false))
            })),
            _ => None,
        };
        let placed = if let Some(p) = part_place { p } else { match (target, line) {
            (EditTarget::Shape(id), Some((ops, idx))) => d.shape(id).and_then(|s| {
                let c = render::compartments(s, &opts, &look)?;
                let (y0, _) = if ops { c.ops? } else { c.attr? };
                let r = cam.rect(s);
                let y = cam.pt(0.0, s.y + y0 + 4.0 + idx as f32 * c.lh).y - 3.0;
                Some((Pos2::new(r.left() + 2.0, y), (r.width() - 4.0).max(160.0), false, true))
            }),
            (EditTarget::Shape(id), None) => d.shape(id).map(|s| {
                let r = cam.rect(s);
                let w = r.width().clamp(120.0, 320.0);
                let y = if s.kind.is_pseudo() { r.bottom() + 2.0 } else { r.top() + 4.0 };
                if s.kind == ShapeKind::Text {
                    // free text is edited in place, over the text itself
                    return (r.left_top(), r.width().max(160.0), true, true);
                }
                (Pos2::new(r.center().x - w / 2.0, y), w, s.kind == ShapeKind::Note, false)
            }),
            (EditTarget::Conn(id), _) => d.connectors.iter().find(|c| c.id == id).and_then(|c| {
                let (a, b) = (d.shape(c.from)?, d.shape(c.to)?);
                let path = render::conn_path(d.kind, c, a, b, ds);
                let k = path.len() / 2;
                let (p0, p1) = (path[k.saturating_sub(1)], path[k]);
                let mid = cam.pt((p0.0 + p1.0) / 2.0, (p0.1 + p1.1) / 2.0);
                Some((mid - Vec2::new(80.0, 26.0), 160.0, false, false))
            }),
        } };
        let Some((pos, width, multiline, left)) = placed else {
            self.editing = None;
            return;
        };
        let hint = match line {
            Some((false, _)) => "-name : Type",
            Some((true, _)) => "+operation() : void",
            None => "",
        };
        let mut commit = false;
        let mut cancel = false;
        let ed = self.editing.as_mut().unwrap();
        // the editor never leaves the canvas (over the menus or toolbar) when its shape is
        // partly out of view
        let cr = self.canvas_rect;
        let pos = if cr.is_positive() {
            let x = pos.x.min(cr.right() - width - 12.0).max(cr.left() + 2.0);
            let y = pos.y.min(cr.bottom() - if multiline { 70.0 } else { 28.0 }).max(cr.top() + 2.0);
            Pos2::new(x, y)
        } else {
            pos
        };
        // a centred editor (a shape's name) is pinned by its top middle, so it stays centred over
        // its shape whatever width the text box really takes; line editors keep their left edge
        let (anchor, pivot) = if left { (pos, Align2::LEFT_TOP) } else { (Pos2::new(pos.x + width / 2.0, pos.y), Align2::CENTER_TOP) };
        egui::Area::new(egui::Id::new("inline_edit")).pivot(pivot).fixed_pos(anchor).order(egui::Order::Foreground).show(ctx, |ui| {
            let te = if multiline {
                egui::TextEdit::multiline(&mut ed.text).desired_width(width).desired_rows(3)
            } else {
                let align = if left { egui::Align::Min } else { egui::Align::Center };
                egui::TextEdit::singleline(&mut ed.text).desired_width(width).horizontal_align(align).hint_text(hint)
            };
            let r = ui.add(te);
            if ed.focus {
                r.request_focus();
                ed.focus = false;
            }
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                cancel = true;
            } else if r.lost_focus() {
                commit = true;
            }
        });
        if cancel {
            self.cancel_edit();
        } else if commit {
            self.commit_edit();
        }
    }

    /// Show or hide the arrow at one end by changing the ends' navigability.
    /// The draggable name label under a screen point (the topmost one).
    pub(super) fn label_at(&self, p: Pos2) -> Option<LabelTarget> {
        self.label_rects.iter().rev().find(|(_, r)| r.contains(p)).map(|(t, _)| *t)
    }
}
