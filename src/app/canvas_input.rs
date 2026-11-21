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

        // start of a primary drag
        if resp.drag_started_by(PointerButton::Primary) {
            let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or_default();
            let dp = cam.to_diagram(origin);
            self.commit_edit();
            match self.tool {
                Tool::Conn(kind) => {
                    if kind == ConnKind::Anchor {
                        // a note can be attached to a shape or to a line (e.g. a message)
                        if let Some(EditTarget::Shape(id) | EditTarget::Conn(id)) = self.hit_any(cam, origin) {
                            self.drag = Drag::Connect { from: id, start: dp, cur: origin };
                        }
                    } else if let Some(id) = self.hit_shape(d, cam, origin, true) {
                        self.drag = Drag::Connect { from: id, start: dp, cur: origin };
                    }
                }
                Tool::Shape(_) => {}
                Tool::Select => {
                    let mut handled = false;
                    // a section line of an alternative fragment?
                    if !handled {
                        if let Some((id, idx, y)) = divider_at(d, cam, origin) {
                            self.checkpoint();
                            if let Some(s) = self.diag_mut().and_then(|dm| dm.shape_mut(id)) {
                                s.dividers = Some(s.alt_dividers());
                            }
                            self.clear_selection();
                            self.sel_shapes.push(id);
                            self.drag = Drag::Divider { id, idx, orig: y };
                            handled = true;
                        }
                    }
                    // the line between two partitions of a swimlane (not over a shape on it)?
                    if !handled && !Self::over_plain_shape(d, cam, origin) {
                        if let Some((id, idx)) = partition_edge_at(d, cam, origin) {
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
                            handled = true;
                        }
                    }
                    // the top or bottom handle of the selected activation?
                    if let Some((r, top, bottom)) = self.bar_handles(d, cam) {
                        let pick = if (origin - top).length() < 8.0 { Some(true) } else if (origin - bottom).length() < 8.0 { Some(false) } else { None };
                        if let (Some(is_top), Some((lid, depth, _))) = (pick, self.sel_bar) {
                            if let Some(s) = d.shape(lid) {
                                self.checkpoint();
                                let (y0, y1) = (cam.to_diagram(r.left_top()).1, cam.to_diagram(r.left_bottom()).1);
                                self.drag = Drag::BarEdge { lifeline: lid, depth, top: is_top, y0, y1, splits0: s.splits.clone(), spans0: s.act_spans.clone() };
                                handled = true;
                            }
                        }
                    }
                    // an end of the selected line?
                    if self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                        if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                            if let Some(ends) = self.end_handles(d, cam, c) {
                                if let Some(k) = ends.iter().position(|e| (*e - origin).length() < 8.0) {
                                    self.checkpoint();
                                    self.drag = Drag::MoveEnd { id: c.id, to_end: k == 1 };
                                    handled = true;
                                }
                            }
                        }
                    }
                    // a bend point (or a segment's middle dot) of the selected line?
                    if !handled && self.sel_conns.len() == 1 && self.sel_shapes.is_empty() {
                        if let Some(c) = d.connectors.iter().find(|c| c.id == self.sel_conns[0]) {
                            if let Some(hit) = Self::point_handle_hit(d, cam, c, origin) {
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
                                handled = true;
                            }
                        }
                    }
                    // a resize handle of the single selected shape?
                    if self.sel_shapes.len() == 1 {
                        if let Some(s) = d.shape(self.sel_shapes[0]) {
                            if !s.kind.fixed_size() {
                                for (i, h) in render::handle_points(cam, s).iter().enumerate() {
                                    if (origin - *h).length() < 7.0 {
                                        self.checkpoint();
                                        self.drag = Drag::Resize { id: s.id, corner: i, orig: (s.x, s.y, s.w, s.h), start: dp };
                                        handled = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    // a name label (of a line, or below a small shape)?
                    if !handled {
                        if let Some(t) = self.label_at(origin) {
                            let orig = match t {
                                LabelTarget::Conn(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.label_offset),
                                LabelTarget::ConnFrom(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.from_label_offset),
                                LabelTarget::ConnTo(id) => d.connectors.iter().find(|c| c.id == id).map(|c| c.to_label_offset),
                                LabelTarget::Shape(id) => d.shape(id).map(|s| s.label_offset),
                            };
                            if let Some(orig) = orig {
                                self.clear_selection();
                                match t.owner() {
                                    EditTarget::Conn(id) => self.sel_conns.push(id),
                                    EditTarget::Shape(id) => self.sel_shapes.push(id),
                                }
                                self.checkpoint();
                                self.drag = Drag::MoveLabel { target: t, start: dp, orig };
                                handled = true;
                            }
                        }
                    }
                    if !handled {
                        match self.hit_any(cam, origin) {
                            Some(EditTarget::Shape(id)) => {
                                if !self.sel_shapes.contains(&id) {
                                    if !shift {
                                        self.clear_selection();
                                    }
                                    self.sel_shapes.push(id);
                                }
                                self.checkpoint();
                                let mut ids: Vec<u64> = self.sel_shapes.clone();
                                // containers carry what is inside them
                                for sid in self.sel_shapes.clone() {
                                    if let Some(cs) = d.shape(sid) {
                                        if cs.kind.is_container() {
                                            for o in &d.shapes {
                                                if o.id != cs.id && cs.contains_rect(o) && !ids.contains(&o.id) {
                                                    ids.push(o.id);
                                                }
                                            }
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
                }
            }
        }

        // while dragging
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = pointer {
                let dp = cam.to_diagram(p);
                let snap = self.settings.snap;
                let g = self.settings.grid_size.max(1.0);
                let snapv = move |v: f32| if snap { (v / g).round() * g } else { v };
                let mut moves: Vec<(u64, f32, f32)> = vec![];
                let mut resize: Option<(u64, f32, f32, f32, f32)> = None;
                let mut msg: Option<(u64, f32)> = None;
                let mut line_moves: Vec<(u64, Vec<(f32, f32)>)> = vec![];
                let mut point: Option<(u64, usize, (f32, f32))> = None;
                let mut label_move: Option<(LabelTarget, (f32, f32))> = None;
                let mut end_move: Option<(u64, bool)> = None;
                let mut line_move: Option<(u64, (f32, f32), (f32, f32), Vec<(f32, f32)>)> = None;
                let mut bar_edit: Option<(u64, usize, bool, f32, f32, Vec<f32>, Vec<[f32; 3]>, f32)> = None;
                let mut divider: Option<(u64, usize, f32)> = None;
                let mut part_edge: Option<(u64, usize, bool, (f32, f32))> = None;
                let locked: std::collections::HashSet<u64> = if matches!(self.drag, Drag::Move { .. }) { d.shapes.iter().filter(|s| y_locked(&d, s)).map(|s| s.id).collect() } else { Default::default() };
                match &mut self.drag {
                    Drag::Move { start, items, lines } => {
                        let (dx, dy) = (snapv(dp.0 - start.0), snapv(dp.1 - start.1));
                        // lifelines along the top of a sequence diagram move only sideways
                        moves = items.iter().map(|(id, x, y)| (*id, x + dx, if locked.contains(id) { *y } else { y + dy })).collect();
                        line_moves = lines.iter().map(|(id, pts)| (*id, pts.iter().map(|(x, y)| (x + dx, y + dy)).collect())).collect();
                    }
                    Drag::MovePoint { id, idx } => point = Some((*id, *idx, (snapv(dp.0), snapv(dp.1)))),
                    Drag::MoveLabel { target, start, orig } => label_move = Some((*target, (orig.0 + dp.0 - start.0, orig.1 + dp.1 - start.1))),
                    Drag::MoveEnd { id, to_end } => end_move = Some((*id, *to_end)),
                    Drag::MoveLine { id, start, from0, to0, points0 } => {
                        let (dx, dy) = (snapv(dp.0 - start.0), snapv(dp.1 - start.1));
                        line_move = Some((*id, (from0.0 + dx, from0.1 + dy), (to0.0 + dx, to0.1 + dy), points0.iter().map(|(x, y)| (x + dx, y + dy)).collect()));
                    }
                    Drag::Resize { id, corner, orig, start } => {
                        let (dx, dy) = (dp.0 - start.0, dp.1 - start.1);
                        let (ox, oy, ow, oh) = *orig;
                        let (mut x0, mut y0, mut x1, mut y1) = (ox, oy, ox + ow, oy + oh);
                        match *corner {
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
                            if *corner == 0 || *corner == 3 {
                                x0 = x1 - 8.0;
                            } else {
                                x1 = x0 + 8.0;
                            }
                        }
                        if y1 - y0 < 4.0 {
                            if *corner <= 1 {
                                y0 = y1 - 4.0;
                            } else {
                                y1 = y0 + 4.0;
                            }
                        }
                        resize = Some((*id, x0, y0, x1 - x0, y1 - y0));
                    }
                    Drag::Rubber { cur, .. } => *cur = p,
                    Drag::Connect { cur, .. } => *cur = p,
                    Drag::MoveMessage { id, orig, start } => msg = Some((*id, *orig + dp.1 - *start)),
                    Drag::BarEdge { lifeline, depth, top, y0, y1, splits0, spans0 } => bar_edit = Some((*lifeline, *depth, *top, *y0, *y1, splits0.clone(), spans0.clone(), snapv(dp.1))),
                    Drag::Divider { id, idx, .. } => divider = Some((*id, *idx, snapv(dp.1))),
                    Drag::PartEdge { id, idx, vertical, .. } => part_edge = Some((*id, *idx, *vertical, (snapv(dp.0), snapv(dp.1)))),
                    Drag::None => {}
                }
                // keep the bar selected by its edge that doesn't move
                if let Some((lid, depth, top, y0, y1, ..)) = &bar_edit {
                    self.sel_bar = Some((*lid, *depth, if *top { *y1 - 1.0 } else { *y0 + 1.0 }));
                }
                if let Some(dm) = self.diag_mut() {
                    for (id, x, y) in moves {
                        if let Some(s) = dm.shape_mut(id) {
                            s.x = x;
                            s.y = y;
                        }
                        // a created lifeline takes its Create message along (the line meets its head)
                        if dm.kind == DiagramKind::Sequence {
                            if let Some(head) = dm.shape(id).filter(|s| s.kind.is_lifeline()).map(|s| s.y + lifeline_head(s.kind) / 2.0) {
                                for c in dm.connectors.iter_mut().filter(|c| c.kind == ConnKind::CreateMessage && c.to == id) {
                                    c.y = head;
                                }
                            }
                        }
                    }
                    if let Some((id, x, y, w, h)) = resize {
                        if let Some(s) = dm.shape_mut(id) {
                            s.x = x;
                            s.y = y;
                            s.w = w;
                            s.h = h;
                        }
                    }
                    if let Some((id, idx, vertical, (px, py))) = part_edge {
                        if let Some(s) = dm.shape_mut(id) {
                            let along = if vertical { px - s.x } else { py - s.y };
                            s.set_partition_edge(idx, along);
                        }
                    }
                    if let Some((id, idx, ny)) = divider {
                        // a section line stays between its neighbours (and below the fragment's tab)
                        if let Some(s) = dm.shape_mut(id) {
                            set_divider(s, idx, ny);
                        }
                    }
                    if let Some((lid, depth, top, y0, y1, splits0, spans0, ny)) = bar_edit.take() {
                        if let Some(s) = dm.shape_mut(lid) {
                            s.splits = splits0;
                            s.act_spans = spans0;
                            stretch_activation(s, depth, top, y0, y1, ny);
                        }
                    }
                    if let Some((id, y)) = msg {
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
                    for (id, pts) in line_moves {
                        if let Some(c) = dm.conn_mut(id) {
                            c.points = pts;
                        }
                    }
                    if let Some((id, f, t, pts)) = line_move {
                        // each end goes to the point of its shape's outline nearest its moved place
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
                    if let Some((id, to_end)) = end_move {
                        // slide the end along its shape's outline, to the point nearest the mouse
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
                    match label_move {
                        Some((LabelTarget::Conn(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.label_offset = off;
                            }
                        }
                        Some((LabelTarget::Shape(id), off)) => {
                            if let Some(s) = dm.shape_mut(id) {
                                s.label_offset = off;
                            }
                        }
                        Some((LabelTarget::ConnFrom(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.from_label_offset = off;
                            }
                        }
                        Some((LabelTarget::ConnTo(id), off)) => {
                            if let Some(c) = dm.conn_mut(id) {
                                c.to_label_offset = off;
                            }
                        }
                        None => {}
                    }
                    if let Some((id, idx, p)) = point {
                        if let Some(c) = dm.conn_mut(id) {
                            if let Some(slot) = c.points.get_mut(idx) {
                                *slot = p;
                            }
                        }
                    }
                }
            }
        }

        if resp.drag_stopped_by(PointerButton::Primary) {
            let drag = std::mem::replace(&mut self.drag, Drag::None);
            match drag {
                Drag::Rubber { start, cur } => {
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
                Drag::Connect { from, start, cur } => {
                    if let Tool::Conn(kind) = self.tool {
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
                }
                Drag::PartEdge { id, orig, .. } => {
                    let now: Vec<f32> = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.lane_partitions().iter().map(|p| p.size).collect()).unwrap_or_default();
                    if now == orig {
                        self.undo.pop();
                    }
                }
                Drag::Divider { id, idx, orig } => {
                    let moved = self.diag().and_then(|dm| dm.shape(id)).map(|s| s.alt_dividers().get(idx).map(|v| (v.y - orig).abs() > 0.01).unwrap_or(false)).unwrap_or(false);
                    if !moved {
                        self.undo.pop();
                    }
                }
                Drag::BarEdge { lifeline, splits0, spans0, .. } => {
                    let same = self.diag().and_then(|dm| dm.shape(lifeline)).map(|s| s.splits == splits0 && s.act_spans == spans0).unwrap_or(true);
                    if same {
                        self.undo.pop();
                    }
                }
                Drag::Move { items, .. } => {
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
                    }
                }
                _ => {}
            }
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
