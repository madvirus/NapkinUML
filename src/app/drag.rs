//! Primary-button drags on the canvas: starting one, following the mouse, and finishing it.
//! Each `Drag` variant has its own start_*, update_* and finish_* function.

use super::*;

type MoveItems = Vec<(u64, f32, f32)>;
type MoveLines = Vec<(u64, Vec<(f32, f32)>)>;

impl UmlApp {
    /// The grid snap of the settings, applied to one coordinate.
    fn snapv(&self, v: f32) -> f32 {
        let g = self.settings.grid_size.max(1.0);
        if self.settings.snap {
            (v / g).round() * g
        } else {
            v
        }
    }

    // ---- start ----

    pub(super) fn drag_start(&mut self, ctx: &egui::Context, cam: &Cam, d: &Diagram, shift: bool) {
        let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or_default();
        let dp = cam.to_diagram(origin);
        self.commit_edit();
        match self.tool {
            Tool::Conn(kind) => self.start_connect(cam, d, kind, origin, dp),
            Tool::Shape(_) => {}
            Tool::Select => self.start_select(cam, d, origin, dp, shift),
        }
    }

    fn start_connect(&mut self, cam: &Cam, d: &Diagram, kind: ConnKind, origin: Pos2, dp: (f32, f32)) {
        if kind == ConnKind::Anchor {
            // a note can be attached to a shape or to a line (e.g. a message)
            if let Some(EditTarget::Shape(id) | EditTarget::Conn(id)) = self.hit_any(cam, origin) {
                self.drag = Drag::Connect { from: id, start: dp, cur: origin };
            }
        } else if let Some(id) = self.hit_shape(d, cam, origin, true) {
            self.drag = Drag::Connect { from: id, start: dp, cur: origin };
        }
    }

    /// Tries the handles in order. Some steps don't check `handled`, so a later handle takes
    /// over one found before it (the bar handle, the line's end and the resize handle).
    fn start_select(&mut self, cam: &Cam, d: &Diagram, origin: Pos2, dp: (f32, f32), shift: bool) {
        let mut handled = self.start_divider(cam, d, origin);
        if !handled && !Self::over_plain_shape(d, cam, origin) {
            handled = self.start_part_edge(cam, d, origin);
        }
        handled |= self.start_bar_edge(cam, d, origin);
        handled |= self.start_move_end(cam, d, origin);
        if !handled {
            handled = self.start_move_point(cam, d, origin, dp);
        }
        handled |= self.start_resize(cam, d, origin, dp);
        if !handled {
            handled = self.start_move_label(d, origin, dp);
        }
        if !handled {
            self.start_by_hit(cam, d, origin, dp, shift);
        }
    }

    /// A section line of an alternative fragment?
    fn start_divider(&mut self, cam: &Cam, d: &Diagram, origin: Pos2) -> bool {
        let Some((id, idx, y)) = divider_at(d, cam, origin) else { return false };
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
            s.dividers = Some(s.alt_dividers());
        }
        self.clear_selection();
        self.sel_shapes.push(id);
        self.drag = Drag::Divider { id, idx, orig: y };
        true
    }

    /// The line between two partitions of a swimlane (the caller checks it is not over a shape on it)?
    fn start_part_edge(&mut self, cam: &Cam, d: &Diagram, origin: Pos2) -> bool {
        let Some((id, idx)) = partition_edge_at(d, cam, origin) else { return false };
        self.checkpoint();
        let mut vertical = true;
        let mut orig = vec![];
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
            s.normalize_partitions();
            vertical = s.kind == ShapeKind::VerticalSwimlane;
            orig = s.lane_partitions().iter().map(|p| p.size).collect();
        }
        self.clear_selection();
        self.sel_shapes.push(id);
        self.drag = Drag::PartEdge { id, idx, vertical, orig };
        true
    }

    /// The top or bottom handle of the selected activation?
    fn start_bar_edge(&mut self, cam: &Cam, d: &Diagram, origin: Pos2) -> bool {
        let Some((r, top, bottom)) = self.bar_handles(d, cam) else { return false };
        let pick = if (origin - top).length() < 8.0 { Some(true) } else if (origin - bottom).length() < 8.0 { Some(false) } else { None };
        let (Some(is_top), Some((lid, depth, _))) = (pick, self.sel_bar) else { return false };
        let Some(s) = d.shape(lid) else { return false };
        self.checkpoint();
        let (y0, y1) = (cam.to_diagram(r.left_top()).1, cam.to_diagram(r.left_bottom()).1);
        self.drag = Drag::BarEdge { lifeline: lid, depth, top: is_top, y0, y1, splits0: s.splits.clone(), spans0: s.act_spans.clone() };
        true
    }

    /// An end of the selected line?
    fn start_move_end(&mut self, cam: &Cam, d: &Diagram, origin: Pos2) -> bool {
        if !(self.sel_conns.len() == 1 && self.sel_shapes.is_empty()) {
            return false;
        }
        let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) else { return false };
        let Some(ends) = self.end_handles(d, cam, c) else { return false };
        let Some(k) = ends.iter().position(|e| (*e - origin).length() < 8.0) else { return false };
        self.checkpoint();
        self.drag = Drag::MoveEnd { id: c.id, to_end: k == 1 };
        true
    }

    /// A bend point (or a segment's middle dot) of the selected line?
    fn start_move_point(&mut self, cam: &Cam, d: &Diagram, origin: Pos2, dp: (f32, f32)) -> bool {
        if !(self.sel_conns.len() == 1 && self.sel_shapes.is_empty()) {
            return false;
        }
        let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) else { return false };
        let Some(hit) = Self::point_handle_hit(d, cam, c, origin) else { return false };
        self.checkpoint();
        let cid = c.id;
        let idx = match hit {
            PointHit::Point(i) => i,
            PointHit::Mid(i) => {
                if let Some(cm) = self.diag_mut().and_then(|dm| dm.conn_mut(cid)) {
                    cm.points.insert(i, dp);
                }
                i
            }
        };
        self.drag = Drag::MovePoint { id: cid, idx };
        true
    }

    /// A resize handle of the single selected shape?
    fn start_resize(&mut self, cam: &Cam, d: &Diagram, origin: Pos2, dp: (f32, f32)) -> bool {
        if self.sel_shapes.len() != 1 {
            return false;
        }
        let Some(s) = d.shape(self.sel_shapes[0]) else { return false };
        if s.kind.fixed_size() {
            return false;
        }
        for (i, h) in render::handle_points(cam, s).iter().enumerate() {
            if (origin - *h).length() < 7.0 {
                self.checkpoint();
                self.drag = Drag::Resize { id: s.id, corner: i, orig: (s.x, s.y, s.w, s.h), start: dp };
                return true;
            }
        }
        false
    }

    /// A name label (of a line, or below a small shape)?
    fn start_move_label(&mut self, d: &Diagram, origin: Pos2, dp: (f32, f32)) -> bool {
        let Some(t) = self.label_at(origin) else { return false };
        let orig = match t {
            LabelTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.label_offset),
            LabelTarget::ConnFrom(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.from_label_offset),
            LabelTarget::ConnTo(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.to_label_offset),
            LabelTarget::Shape(id) => d.shape(id).map(|s| s.label_offset),
        };
        let Some(orig) = orig else { return false };
        self.clear_selection();
        match t.owner() {
            EditTarget::Conn(id) => self.sel_conns.push(id),
            EditTarget::Shape(id) => self.sel_shapes.push(id),
        }
        self.checkpoint();
        self.drag = Drag::MoveLabel { target: t, start: dp, orig };
        true
    }

    /// What is under the press: a shape (Move), a line (MoveMessage or MoveLine), or nothing (Rubber).
    fn start_by_hit(&mut self, cam: &Cam, d: &Diagram, origin: Pos2, dp: (f32, f32), shift: bool) {
        match self.hit_any(cam, origin) {
            Some(EditTarget::Shape(id)) => {
                if !self.sel_shapes.contains(&id) {
                    if !shift {
                        self.clear_selection();
                    }
                    self.sel_shapes.push(id);
                }
                self.checkpoint();
                let (items, lines) = move_items(d, &self.sel_shapes);
                self.drag = Drag::Move { start: dp, items, lines };
            }
            Some(EditTarget::Conn(id)) => {
                if !shift {
                    self.clear_selection();
                }
                if !self.sel_conns.contains(&id) {
                    self.sel_conns.push(id);
                }
                if let Some(c) = d.connectors.iter().find(|c| c.id == id) {
                    if d.kind == DiagramKind::Sequence && c.kind.is_message() {
                        self.checkpoint();
                        self.drag = Drag::MoveMessage { id, orig: c.y, start: dp.1 };
                    } else if c.from != c.to && d.shape(c.from).is_some() && d.shape(c.to).is_some() {
                        // drag the whole line: both ends slide along their shapes
                        let opts = self.project.as_ref().map(|p| p.options.clone()).unwrap_or_default();
                        let (_, paths) = crate::seq::all_paths(d, &opts);
                        if let Some(p) = paths.get(&id) {
                            if let (Some(&f0), Some(&t0)) = (p.first(), p.last()) {
                                self.checkpoint();
                                self.drag = Drag::MoveLine { id, start: dp, from0: f0, to0: t0, points0: c.points.clone() };
                            }
                        }
                    }
                }
            }
            None => {
                if !shift {
                    self.clear_selection();
                }
                self.drag = Drag::Rubber { start: origin, cur: origin };
            }
        }
    }

    // ---- while dragging ----

    pub(super) fn drag_update(&mut self, cam: &Cam, d: &Diagram, p: Pos2) {
        let dp = cam.to_diagram(p);
        // taken out for the frame, so the handlers below can change the diagram freely
        let mut drag = std::mem::replace(&mut self.drag, Drag::None);
        match &mut drag {
            Drag::Move { start, items, lines } => self.update_move(d, dp, *start, items, lines),
            Drag::MovePoint { id, idx } => self.update_move_point(*id, *idx, dp),
            Drag::MoveLabel { target, start, orig } => self.update_move_label(*target, *start, *orig, dp),
            Drag::MoveEnd { id, to_end } => self.update_move_end(*id, *to_end, dp),
            Drag::MoveLine { id, start, from0, to0, points0 } => self.update_move_line(*id, *start, *from0, *to0, points0, dp),
            Drag::Resize { id, corner, orig, start } => self.update_resize(*id, *corner, *orig, *start, dp),
            Drag::Rubber { cur, .. } => *cur = p,
            Drag::Connect { cur, .. } => *cur = p,
            Drag::MoveMessage { id, orig, start } => self.update_move_message(*id, *orig + dp.1 - *start),
            Drag::BarEdge { lifeline, depth, top, y0, y1, splits0, spans0 } => self.update_bar_edge(*lifeline, *depth, *top, *y0, *y1, splits0, spans0, dp),
            Drag::Divider { id, idx, .. } => self.update_divider(*id, *idx, dp),
            Drag::PartEdge { id, idx, vertical, .. } => self.update_part_edge(*id, *idx, *vertical, dp),
            Drag::None => {}
        }
        self.drag = drag;
    }

    fn update_move(&mut self, d: &Diagram, dp: (f32, f32), start: (f32, f32), items: &[(u64, f32, f32)], lines: &[(u64, Vec<(f32, f32)>)]) {
        let locked: std::collections::HashSet<u64> = d.shapes.iter().filter(|s| y_locked(d, s)).map(|s| s.id).collect();
        let (dx, dy) = (self.snapv(dp.0 - start.0), self.snapv(dp.1 - start.1));
        let Some(dm) = self.diag_mut() else { return };
        for (id, x, y) in items {
            if let Some(s) = dm.shape_mut(*id) {
                s.x = x + dx;
                // lifelines along the top of a sequence diagram move only sideways
                s.y = if locked.contains(id) { *y } else { y + dy };
            }
            // a created lifeline takes its Create message along (the line meets its head)
            if dm.kind == DiagramKind::Sequence {
                if let Some(head) = dm.shape(*id).filter(|s| s.kind.is_lifeline()).map(|s| s.y + lifeline_head(s.kind) / 2.0) {
                    for c in dm.connectors.iter_mut().filter(|c| c.kind == ConnKind::CreateMessage && c.to == *id) {
                        c.y = head;
                    }
                }
            }
        }
        // bent lines between moved shapes move along
        for (id, pts) in lines {
            if let Some(c) = dm.conn_mut(*id) {
                c.points = pts.iter().map(|(x, y)| (x + dx, y + dy)).collect();
            }
        }
    }

    fn update_move_point(&mut self, id: u64, idx: usize, dp: (f32, f32)) {
        let p = (self.snapv(dp.0), self.snapv(dp.1));
        if let Some(slot) = self.diag_mut().and_then(|dm| dm.conn_mut(id)).and_then(|c| c.points.get_mut(idx)) {
            *slot = p;
        }
    }

    fn update_move_label(&mut self, target: LabelTarget, start: (f32, f32), orig: (f32, f32), dp: (f32, f32)) {
        let off = (orig.0 + dp.0 - start.0, orig.1 + dp.1 - start.1);
        let Some(dm) = self.diag_mut() else { return };
        match target {
            LabelTarget::Conn(id) => {
                if let Some(c) = dm.conn_mut(id) {
                    c.label_offset = off;
                }
            }
            LabelTarget::Shape(id) => {
                if let Some(s) = dm.shape_mut(id) {
                    s.label_offset = off;
                }
            }
            LabelTarget::ConnFrom(id) => {
                if let Some(c) = dm.conn_mut(id) {
                    c.from_label_offset = off;
                }
            }
            LabelTarget::ConnTo(id) => {
                if let Some(c) = dm.conn_mut(id) {
                    c.to_label_offset = off;
                }
            }
        }
    }

    /// Slides the end along its shape's outline, to the point nearest the mouse.
    fn update_move_end(&mut self, id: u64, to_end: bool, dp: (f32, f32)) {
        let Some(dm) = self.diag_mut() else { return };
        let sid = dm.connectors.iter().find(|c| c.id == id).map(|c| if to_end { c.to } else { c.from });
        if let Some(s) = sid.and_then(|s| dm.shape(s)).cloned() {
            let f = render::nearest_anchor(&s, dp);
            if let Some(c) = dm.conn_mut(id) {
                if to_end {
                    c.to_anchor = Some(f);
                } else {
                    c.from_anchor = Some(f);
                }
            }
        }
    }

    /// Each end goes to the point of its shape's outline nearest its moved place.
    fn update_move_line(&mut self, id: u64, start: (f32, f32), from0: (f32, f32), to0: (f32, f32), points0: &[(f32, f32)], dp: (f32, f32)) {
        let (dx, dy) = (self.snapv(dp.0 - start.0), self.snapv(dp.1 - start.1));
        let (f, t) = ((from0.0 + dx, from0.1 + dy), (to0.0 + dx, to0.1 + dy));
        let pts: Vec<(f32, f32)> = points0.iter().map(|(x, y)| (x + dx, y + dy)).collect();
        let Some(dm) = self.diag_mut() else { return };
        let ends = dm.connectors.iter().find(|c| c.id == id).map(|c| (c.from, c.to));
        if let Some((fs, ts)) = ends {
            let fa = dm.shape(fs).map(|s| render::nearest_anchor(s, f));
            let ta = dm.shape(ts).map(|s| render::nearest_anchor(s, t));
            if let Some(c) = dm.conn_mut(id) {
                if fa.is_some() {
                    c.from_anchor = fa;
                }
                if ta.is_some() {
                    c.to_anchor = ta;
                }
                c.points = pts;
            }
        }
    }

    fn update_resize(&mut self, id: u64, corner: usize, orig: (f32, f32, f32, f32), start: (f32, f32), dp: (f32, f32)) {
        let (x, y, w, h) = resize_rect(corner, orig, (dp.0 - start.0, dp.1 - start.1), |v| self.snapv(v));
        // a timeline axis made longer or shorter carries its events along in proportion
        if let Some(dm) = self.diag_mut() {
            if let Some(mut a) = dm.shape(id).filter(|s| s.kind == ShapeKind::TimeAxis).cloned() {
                (a.x, a.y, a.w, a.h) = (x, y, w, h);
                crate::model::stretch_time_axis(dm, a);
                return;
            }
        }
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
            s.x = x;
            s.y = y;
            s.w = w;
            s.h = h;
        }
    }

    fn update_move_message(&mut self, id: u64, y: f32) {
        let Some(dm) = self.diag_mut() else { return };
        if let Some(c) = dm.conn_mut(id) {
            c.y = y;
        }
        // moving a Create message moves the head of the lifeline it creates (its bottom stays)
        let target = dm.connectors.iter().find(|c| c.id == id && c.kind == ConnKind::CreateMessage && c.from != c.to).map(|c| c.to);
        if let Some(s) = target.and_then(|t| dm.shape_mut(t)) {
            if s.kind.is_lifeline() {
                let bottom = s.y + s.h;
                s.y = y - lifeline_head(s.kind) / 2.0;
                s.h = (bottom - s.y).max(lifeline_head(s.kind) + 60.0);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update_bar_edge(&mut self, lid: u64, depth: usize, top: bool, y0: f32, y1: f32, splits0: &[f32], spans0: &[[f32; 3]], dp: (f32, f32)) {
        let ny = self.snapv(dp.1);
        // keep the bar selected by its edge that doesn't move
        self.sel_bar = Some((lid, depth, if top { y1 - 1.0 } else { y0 + 1.0 }));
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(lid)) {
            s.splits = splits0.to_vec();
            s.act_spans = spans0.to_vec();
            stretch_activation(s, depth, top, y0, y1, ny);
        }
    }

    /// A section line stays between its neighbours (and below the fragment's tab).
    fn update_divider(&mut self, id: u64, idx: usize, dp: (f32, f32)) {
        let ny = self.snapv(dp.1);
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
            set_divider(s, idx, ny);
        }
    }

    fn update_part_edge(&mut self, id: u64, idx: usize, vertical: bool, dp: (f32, f32)) {
        let (px, py) = (self.snapv(dp.0), self.snapv(dp.1));
        if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
            let along = if vertical { px - s.x } else { py - s.y };
            s.set_partition_edge(idx, along);
        }
    }

    // ---- finish ----

    pub(super) fn drag_finish(&mut self, cam: &Cam, d: &Diagram) {
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::Rubber { start, cur } => self.finish_rubber(cam, d, start, cur),
            Drag::Connect { from, start, cur } => self.finish_connect(cam, d, from, start, cur),
            Drag::PartEdge { id, orig, .. } => self.finish_part_edge(id, &orig),
            Drag::Divider { id, idx, orig } => self.finish_divider(id, idx, orig),
            Drag::BarEdge { lifeline, splits0, spans0, .. } => self.finish_bar_edge(lifeline, &splits0, &spans0),
            Drag::Move { items, .. } => self.finish_move(&items),
            _ => {}
        }
    }

    fn finish_rubber(&mut self, cam: &Cam, d: &Diagram, start: Pos2, cur: Pos2) {
        let r = Rect::from_two_pos(start, cur);
        for s in &d.shapes {
            if r.contains_rect(cam.rect(s)) && !self.sel_shapes.contains(&s.id) {
                self.sel_shapes.push(s.id);
            }
        }
        for c in &d.connectors {
            if self.sel_shapes.contains(&c.from) && self.sel_shapes.contains(&c.to) && !self.sel_conns.contains(&c.id) {
                self.sel_conns.push(c.id);
            }
        }
    }

    fn finish_connect(&mut self, cam: &Cam, d: &Diagram, from: u64, start: (f32, f32), cur: Pos2) {
        let Tool::Conn(kind) = self.tool else { return };
        if kind == ConnKind::Anchor {
            if let Some(EditTarget::Shape(to) | EditTarget::Conn(to)) = self.hit_any(cam, cur) {
                self.create_anchor(from, to);
            }
        } else if let (true, Some(src)) = (d.kind == DiagramKind::Sequence && matches!(kind, ConnKind::Message | ConnKind::AsyncMessage | ConnKind::CreateMessage), d.shape(from).filter(|s| s.kind.is_lifeline())) {
            // a sequence message: to another lifeline; back onto (or near) its own
            // lifeline = a self message; onto empty space = a new lifeline there.
            // Fragments it is dropped inside don't count.
            let x = cam.to_diagram(cur).0;
            let target = self.hit_shape(d, cam, cur, true).filter(|id| d.shape(*id).map(|s| !s.kind.is_container()).unwrap_or(false));
            let near_self = (x - (src.x + src.w / 2.0)).abs() < SELF_REACH;
            match target {
                Some(to) if to != from => self.create_connector(kind, from, to, start.1),
                Some(_) if kind != ConnKind::CreateMessage => self.create_connector(kind, from, from, start.1),
                None if near_self && kind != ConnKind::CreateMessage => self.create_connector(kind, from, from, start.1),
                None if !near_self => self.message_to_new_lifeline(kind, from, x, start.1),
                _ => {}
            }
        } else if let (None, Some(default)) = (
            // a swimlane's or network zone's empty area counts as empty space
            self.hit_shape(d, cam, cur, true).filter(|id| !d.shape(*id).map(|s| counts_as_empty(s.kind)).unwrap_or(false)),
            d.shape(from).and_then(|s| pick_default(d.kind, kind, s.kind)),
        ) {
            // Connect and Create: ask what to make there first (see conn_picker)
            let src_kind = d.shape(from).map(|s| s.kind).unwrap_or(default);
            let sel = pick_choices(d.kind, kind, src_kind).iter().position(|(k, _)| *k == default).unwrap_or(0);
            self.conn_pick = Some(ConnPick { conn: kind, from, at: cam.to_diagram(cur), sel });
        } else if let Some(to) = self.hit_shape(d, cam, cur, true) {
            let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
            self.create_connector(kind, from, to, start.1);
            // where the drag started picks the line's lane between two boxes
            if !(d.kind == DiagramKind::Sequence && kind.is_message()) && from != to {
                if let (Some(a), Some(b)) = (d.shape(from).cloned(), d.shape(to).cloned()) {
                    if let Some((fa, fb)) = press_lane(&a, &b, start) {
                        if let Some(dm) = self.diag_mut() {
                            if dm.connectors.len() > before {
                                if let Some(c) = dm.connectors.last_mut() {
                                    c.from_anchor = Some(fa);
                                    c.to_anchor = Some(fb);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn finish_part_edge(&mut self, id: u64, orig: &[f32]) {
        let now: Vec<f32> = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.lane_partitions().iter().map(|p| p.size).collect()).unwrap_or_default();
        if now == orig {
            self.undo.pop();
        }
    }

    fn finish_divider(&mut self, id: u64, idx: usize, orig: f32) {
        let moved = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.alt_dividers().get(idx).map(|v| (v.y - orig).abs() > 0.01).unwrap_or(false)).unwrap_or(false);
        if !moved {
            self.undo.pop();
        }
    }

    fn finish_bar_edge(&mut self, lifeline: u64, splits0: &[f32], spans0: &[[f32; 3]]) {
        let same = self.diag().and_then(|dm| dm.shape(lifeline)).map(|s| s.splits == splits0 && s.act_spans == spans0).unwrap_or(true);
        if same {
            self.undo.pop();
        }
    }

    fn finish_move(&mut self, items: &[(u64, f32, f32)]) {
        // nothing moved: drop the undo step again
        let moved = self
            .diag()
            .map(|dm| items.iter().any(|(id, x, y)| dm.shape(*id).map(|s| s.x != *x || s.y != *y).unwrap_or(false)))
            .unwrap_or(false);
        if !moved {
            self.undo.pop();
        } else {
            if items.len() == 1 {
                // a port dropped near a component attaches to its edge
                self.snap_port_to_owner(items[0].0);
            }
            // a ball dropped near a socket (or a socket near a ball) joins it
            let ids: Vec<u64> = items.iter().map(|(id, _, _)| *id).collect();
            self.join_interfaces(&ids);
            // a dragged mind map topic joins the topic it was dropped next to
            if self.diag().map(|dm| dm.kind == DiagramKind::MindMap).unwrap_or(false) {
                self.mind_drop();
            }
        }
    }
}

/// What a Move drag of the selected shapes carries: the shapes with where they were, and the
/// bent lines between them with their bend points.
fn move_items(d: &Diagram, sel: &[u64]) -> (MoveItems, MoveLines) {
    let mut ids: Vec<u64> = sel.to_vec();
    // containers carry what is inside them
    for sid in sel {
        if let Some(cs) = d.shape(*sid) {
            if cs.kind.is_container() {
                for o in &d.shapes {
                    if o.id != cs.id && cs.contains_rect(o) && !ids.contains(&o.id) {
                        ids.push(o.id);
                    }
                }
            }
        }
    }
    // a mind map topic carries its sub-topics
    if d.kind == DiagramKind::MindMap {
        for id in crate::model::mind_subtree(d, &ids) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    // ports attached to a moved shape move with it
    for sid in ids.clone() {
        if let Some(cs) = d.shape(sid) {
            for pid in attached_ports(d, cs) {
                if !ids.contains(&pid) {
                    ids.push(pid);
                }
            }
        }
    }
    // a joined ball and socket move as one
    for sid in ids.clone() {
        if let Some(cs) = d.shape(sid) {
            for jid in joined_interfaces(d, cs) {
                if !ids.contains(&jid) {
                    ids.push(jid);
                }
            }
        }
    }
    let items = ids.iter().filter_map(|i| d.shape(*i)).map(|s| (s.id, s.x, s.y)).collect();
    // bent lines between moved shapes move along
    let lines = d
        .connectors
        .iter()
        .filter(|c| !c.points.is_empty() && ids.contains(&c.from) && ids.contains(&c.to))
        .map(|c| (c.id, c.points.clone()))
        .collect();
    (items, lines)
}

/// The shape's new box while a corner handle is dragged by (dx, dy): the dragged corner snaps,
/// and the box keeps at least 8 wide and 4 high.
fn resize_rect(corner: usize, orig: (f32, f32, f32, f32), (dx, dy): (f32, f32), snapv: impl Fn(f32) -> f32) -> (f32, f32, f32, f32) {
    let (ox, oy, ow, oh) = orig;
    let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + ow, oy + oh);
    match corner {
        0 => {
            x0 = snapv(ox + dx);
            y0 = snapv(oy + dy);
        }
        1 => {
            x1 = snapv(ox + ow + dx);
            y0 = snapv(oy + dy);
        }
        2 => {
            x1 = snapv(ox + ow + dx);
            y1 = snapv(oy + oh + dy);
        }
        _ => {
            x0 = snapv(ox + dx);
            y1 = snapv(oy + oh + dy);
        }
    }
    if x1 - x0 < 8.0 {
        if corner == 0 || corner == 3 {
            x0 = x1 - 8.0;
        } else {
            x1 = x0 + 8.0;
        }
    }
    if y1 - y0 < 4.0 {
        if corner <= 1 {
            y0 = y1 - 4.0;
        } else {
            y1 = y0 + 4.0;
        }
    }
    (x0, y0, x1 - x0, y1 - y0)
}
