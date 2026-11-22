//! Editing the diagram: creating shapes and lines, ports, clipboard, selection, alignment and line styles.

use super::*;

impl UmlApp {
    pub(super) fn create_shape(&mut self, kind: ShapeKind, x: f32, y: f32) {
        let snap = self.settings.snap;
        let g = self.settings.grid_size.max(1.0);
        // the part of the diagram the canvas shows now (diagram units)
        let visible = {
            let r = self.canvas_rect;
            let v = self.view();
            (r.is_positive() && r.is_finite() && v.zoom > 0.0).then(|| (-v.pan.x / v.zoom, -v.pan.y / v.zoom, (r.width() - v.pan.x) / v.zoom, (r.height() - v.pan.y) / v.zoom))
        };
        let Some(active) = self.active else { return };
        self.checkpoint();
        let Some(p) = self.project.as_mut() else { return };
        let id = p.fresh_id();
        let Some(d) = p.diagram_mut(active) else { return };
        let dkind = d.kind;
        let (w, h) = kind.default_size();
        let mut sx = x - w / 2.0;
        let mut sy = y - h / 2.0;
        // a shape centred on the click stays inside the visible canvas (a big one such as a
        // System clicked near the top would otherwise start above it, out of sight); when it is
        // bigger than the view its top-left corner stays in view
        if let Some((vx0, vy0, vx1, vy1)) = visible {
            const M: f32 = 10.0;
            sx = sx.min(vx1 - w - M).max(vx0 + M);
            sy = sy.min(vy1 - h - M).max(vy0 + M);
        }
        if kind.is_lifeline() && dkind == DiagramKind::Sequence {
            // lifelines line up along the top
            if let Some(first) = d.shapes.iter().find(|s| s.kind.is_lifeline()) {
                sy = first.y;
            } else {
                sy = y - 20.0;
            }
        }
        if snap {
            sx = (sx / g).round() * g;
            sy = (sy / g).round() * g;
        }
        let base = palette::base_name(kind);
        let name = if base.is_empty() {
            String::new()
        } else {
            let mut n = 1;
            loop {
                let candidate = if n == 1 { base.to_string() } else { format!("{base}{n}") };
                if !d.shapes.iter().any(|s| s.name == candidate) {
                    break candidate;
                }
                n += 1;
            }
        };
        let attributes = match kind {
            ShapeKind::Entity => vec!["PK id : integer".to_string()],
            ShapeKind::DbView => vec!["id : integer".to_string()],
            _ => vec![],
        };
        d.shapes.push(Shape {
            id,
            kind,
            x: sx,
            y: sy,
            w,
            h,
            name: if kind.is_swimlane() { String::new() } else { name.clone() },
            stereotype: String::new(),
            attributes,
            operations: vec![],
            fill: None,
            documentation: String::new(),
            font: None,
            font_size: None,
            orient: 0,
            hide_icon: false,
            hide_attributes: false,
            hide_operations: false,
            fold: None,
            splits: Vec::new(),
            act_spans: Vec::new(),
            dividers: None,
            partitions: if kind.is_swimlane() {
                let half = if kind == ShapeKind::VerticalSwimlane { w / 2.0 } else { h / 2.0 };
                Some(vec![crate::model::Partition { name: "Partition".into(), size: half }, crate::model::Partition { name: "Partition2".into(), size: half }])
            } else {
                None
            },
            label_offset: (0.0, 0.0),
        });
        self.clear_selection();
        self.sel_shapes.push(id);
        if kind == ShapeKind::Port {
            self.snap_port_to_owner(id);
        }
        let editable = (!kind.is_pseudo() || kind == ShapeKind::Actor || kind.is_network_device()) && !matches!(kind, ShapeKind::AltFragment | ShapeKind::LoopFragment) && !kind.is_swimlane();
        if editable {
            self.editing = Some(InlineEdit { target: EditTarget::Shape(id), text: name, focus: true, line: None, new_line: false, part: None, select_all: false, guard: None });
        }
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    /// A provided interface (ball) or required interface (socket) among the moved shapes that
    /// was dropped overlapping one of the other kind snaps onto it, centre on centre, so the
    /// ball sits in the socket; from then on they move together.
    pub(super) fn join_interfaces(&mut self, moved: &[u64]) {
        let Some(d) = self.diag_mut() else { return };
        for &id in moved {
            let Some(s) = d.shape(id).cloned() else { continue };
            if !is_interface_end(s.kind) || !joined_interfaces(d, &s).is_empty() {
                continue;
            }
            let (cx, cy) = s.center();
            let reach = |o: &Shape| (s.w.max(s.h) + o.w.max(o.h)) / 2.0;
            let target = d
                .shapes
                .iter()
                .filter(|o| !moved.contains(&o.id) && opposite_interface(s.kind, o.kind))
                .map(|o| {
                    let (ox, oy) = o.center();
                    (o.center(), ((ox - cx).powi(2) + (oy - cy).powi(2)).sqrt(), reach(o))
                })
                .filter(|(_, dist, r)| dist <= r)
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            if let Some(((ox, oy), _, _)) = target {
                if let Some(m) = d.shape_mut(id) {
                    m.x = ox - m.w / 2.0;
                    m.y = oy - m.h / 2.0;
                }
                // the socket's cup opens toward the side the ball's line comes from (away from the
                // shape that requires the interface)
                let pair_ids: Vec<u64> = d.shapes.iter().filter(|o| {
                    let (px, py) = o.center();
                    is_interface_end(o.kind) && (px - ox).abs() <= 1.5 && (py - oy).abs() <= 1.5
                }).map(|o| o.id).collect();
                let other_end = |sid: u64| -> Option<(f32, f32)> {
                    d.connectors.iter().find_map(|c| {
                        let o = if c.from == sid { c.to } else if c.to == sid { c.from } else { return None };
                        d.shape(o).filter(|x| !pair_ids.contains(&x.id)).map(|x| x.center())
                    })
                };
                let sock = pair_ids.iter().copied().find(|i| d.shape(*i).map(|x| x.kind == ShapeKind::RequiredInterface).unwrap_or(false));
                let ball = pair_ids.iter().copied().find(|i| d.shape(*i).map(|x| x.kind == ShapeKind::ProvidedInterface).unwrap_or(false));
                // direction the cup should open: toward the provider, or away from the requirer
                let dir = ball.and_then(other_end).map(|(px, py)| (px - ox, py - oy)).or_else(|| sock.and_then(other_end).map(|(qx, qy)| (ox - qx, oy - qy)));
                if let (Some(sid), Some((dx, dy))) = (sock, dir) {
                    let orient = if dx.abs() >= dy.abs() { if dx >= 0.0 { 0 } else { 2 } } else if dy >= 0.0 { 1 } else { 3 };
                    if let Some(m) = d.shape_mut(sid) {
                        m.orient = orient;
                    }
                }
                // both names would sit under the pair on top of each other: the socket's goes above
                let pair: Vec<Shape> = d.shapes.iter().filter(|o| o.id == id || joined_interfaces(d, &s).contains(&o.id) || {
                    let (px, py) = o.center();
                    o.id != id && opposite_interface(s.kind, o.kind) && (px - ox).abs() <= 1.5 && (py - oy).abs() <= 1.5
                }).cloned().collect();
                if pair.len() >= 2 && pair.iter().all(|p| !p.name.is_empty()) {
                    if let Some(sock) = pair.iter().find(|p| p.kind == ShapeKind::RequiredInterface) {
                        if sock.label_offset == (0.0, 0.0) {
                            let lift = -(sock.h + 20.0);
                            if let Some(m) = d.shape_mut(sock.id) {
                                m.label_offset = (0.0, lift);
                            }
                        }
                    }
                }
            }
        }
    }

    /// If the shape is a port near a component (or node, class…), put it on that shape's edge.
    pub(super) fn snap_port_to_owner(&mut self, id: u64) {
        let Some(d) = self.diag_mut() else { return };
        let Some(port) = d.shape(id).cloned() else { return };
        if port.kind != ShapeKind::Port {
            return;
        }
        let Some(owner) = port_owner(d, &port).and_then(|o| d.shape(o).cloned()) else { return };
        if let Some(p) = d.shape_mut(id) {
            snap_port(p, &owner);
        }
    }

    pub(super) fn create_connector(&mut self, kind: ConnKind, from: u64, to: u64, y: f32) {
        let Some(d) = self.diag() else { return };
        let (Some(a), Some(b)) = (d.shape(from), d.shape(to)) else { return };
        if from == to && !kind.allows_self() {
            return;
        }
        if kind.is_message() && d.kind == DiagramKind::Sequence && !(a.kind.is_lifeline() && b.kind.is_lifeline()) {
            self.status = "Messages connect two lifelines".into();
            return;
        }
        if kind.is_erd() && !(matches!(a.kind, ShapeKind::Entity | ShapeKind::DbView) && matches!(b.kind, ShapeKind::Entity | ShapeKind::DbView)) {
            self.status = "Relationships connect two entities".into();
            return;
        }
        // a line to an interface ball is a Realization (vpplatform.jar v.dwx.b.a);
        // a line to a socket is a Usage (estimated). The interface is kept as the target.
        let iface = |k: ShapeKind| matches!(k, ShapeKind::ProvidedInterface | ShapeKind::RequiredInterface);
        let plain = matches!(kind, ConnKind::Association | ConnKind::Link | ConnKind::Wireless | ConnKind::Dependency | ConnKind::Realization | ConnKind::Usage);
        let (from, to, a, b) = if plain && iface(a.kind) && !iface(b.kind) { (to, from, b, a) } else { (from, to, a, b) };
        let kind = if plain && b.kind == ShapeKind::ProvidedInterface {
            ConnKind::Realization
        } else if plain && b.kind == ShapeKind::RequiredInterface {
            ConnKind::Usage
        } else {
            kind
        };
        let head = |s: &Shape| if s.kind == ShapeKind::ActorLifeline { 85.0 } else { 55.0 };
        // a Create message meets the head of the lifeline it creates, so only its sender limits it
        let created = kind == ConnKind::CreateMessage && from != to;
        let min_y = if created { a.y + head(a) } else { (a.y + head(a)).max(b.y + head(b)) };
        let max_y = if created { a.y + a.h - 10.0 } else { (a.y + a.h).min(b.y + b.h) - 10.0 };
        let y = y.clamp(min_y, max_y.max(min_y));
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let id = p.fresh_id();
        let d = p.diagram_mut(self.active.unwrap()).unwrap();
        // new lines start without a name (messages too: no "message()" placeholder)
        let name = String::new();
        d.connectors.push(Connector {
            id,
            kind,
            from,
            to,
            name,
            from_label: String::new(),
            to_label: String::new(),
            y,
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
        self.clear_selection();
        self.sel_conns.push(id);
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    /// A message from a lifeline dropped where there is no lifeline: make one there (centred on
    /// the drop point) and connect them, in one undo step. A Create message's lifeline has its
    /// head at the message; for other messages it joins the lifelines along the top.
    pub(super) fn message_to_new_lifeline(&mut self, kind: ConnKind, from: u64, x: f32, press_y: f32) {
        let Some(d) = self.diag() else { return };
        let Some(src) = d.shape(from).cloned() else { return };
        let head = |s: &Shape| if s.kind == ShapeKind::ActorLifeline { 85.0 } else { 55.0 };
        let msg_y = press_y.clamp(src.y + head(&src), (src.y + src.h - 10.0).max(src.y + head(&src)));
        let top = d.shapes.iter().filter(|s| y_locked(d, s)).map(|s| s.y).fold(f32::INFINITY, f32::min);
        let top = if top.is_finite() { top } else { src.y };
        let bottom = d.shapes.iter().filter(|s| s.kind.is_lifeline()).map(|s| s.y + s.h).fold(src.y + src.h, f32::max);
        let y = if kind == ConnKind::CreateMessage { msg_y - lifeline_head(ShapeKind::Lifeline) / 2.0 } else { top };
        let h = (bottom - y).max(msg_y - y + 60.0);
        let undo_before = self.undo.len();
        self.create_shape(ShapeKind::Lifeline, x, y);
        let Some(&new_id) = self.sel_shapes.first() else { return };
        if let Some(dm) = self.diag_mut() {
            if let Some(s) = dm.shape_mut(new_id) {
                s.x = x - s.w / 2.0;
                s.y = y;
                s.h = h;
            }
        }
        let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
        self.create_connector(kind, from, new_id, msg_y);
        if self.diag().map(|dm| dm.connectors.len()).unwrap_or(0) > before && self.undo.len() > undo_before + 1 {
            // the shape and its message undo together
            self.undo.pop();
        }
        // the new lifeline stays selected, with its name being typed
        self.clear_selection();
        self.sel_shapes.push(new_id);
    }

    /// An Association/Aggregation/Composition dragged from a shape and dropped on empty space:
    /// make a shape of the same type centred there and connect them, in one undo step. The new
    /// shape stays selected with its name being typed.
    pub(super) fn conn_to_new_shape(&mut self, kind: ConnKind, from: u64, shape: ShapeKind, x: f32, y: f32) {
        let undo_before = self.undo.len();
        let keep_tool = self.tool;
        self.create_shape(shape, x, y);
        let Some(&new_id) = self.sel_shapes.first() else { return };
        if new_id == from {
            return;
        }
        // create_shape may have put the tool back to Select; the line still needs its own kind
        self.tool = keep_tool;
        let before = self.diag().map(|dm| dm.connectors.len()).unwrap_or(0);
        self.create_connector(kind, from, new_id, y);
        if self.diag().map(|dm| dm.connectors.len()).unwrap_or(0) > before && self.undo.len() > undo_before + 1 {
            // the shape and its line undo together
            self.undo.pop();
        }
        if !self.sticky {
            self.tool = Tool::Select;
        }
        self.clear_selection();
        self.sel_shapes.push(new_id);
    }

    /// Add a section at the bottom of an alternative fragment (splitting the last section; the
    /// fragment grows if that section is too short). It becomes the [else] section; a section
    /// that was [else] before keeps an empty guard to fill in. Returns the index of the section
    /// whose guard is to be filled in (the new one, or the one that was [else]).
    pub(super) fn add_alt_section(&mut self, id: u64) -> Option<usize> {
        self.checkpoint();
        let s = self.diag_mut().and_then(|d| d.shape_mut(id))?;
        let mut v = s.alt_dividers();
        let top = v.last().map(|d| d.y).unwrap_or(crate::model::ALT_HEAD);
        if s.h - top < 60.0 {
            s.h = top + 80.0;
        }
        let y = (top + s.h) / 2.0;
        let mut fill = v.len();
        let guard = if v.iter().any(|d| d.guard.trim() == "else") {
            if let Some(last) = v.last_mut().filter(|d| d.guard.trim() == "else") {
                last.guard.clear();
                fill = v.len() - 1;
                "else".to_string()
            } else {
                String::new()
            }
        } else {
            "else".to_string()
        };
        v.push(crate::model::Divider { y, guard });
        s.dividers = Some(v);
        Some(fill)
    }

    /// Take a section of an alternative fragment away: the one at `at` (height below the
    /// fragment's top) unless that is the first, else the last one. The section above takes its
    /// room.
    pub(super) fn remove_alt_section(&mut self, id: u64, at: Option<f32>) {
        let count = self.diag().and_then(|d| d.shape(id)).map(|s| s.alt_dividers().len()).unwrap_or(0);
        if count == 0 {
            return;
        }
        self.checkpoint();
        let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(id)) else { return };
        let mut v = s.alt_dividers();
        let idx = at.and_then(|y| v.iter().rposition(|d| d.y <= y)).unwrap_or(v.len() - 1);
        v.remove(idx);
        s.dividers = Some(v);
    }

    /// Insert a partition into a swimlane after partition `after`; the swimlane grows and what
    /// stood in the partitions after it moves along with them. Returns the new partition's index.
    pub(super) fn add_partition(&mut self, id: u64, after: usize) -> Option<usize> {
        self.checkpoint();
        let dm = self.diag_mut()?;
        let before = dm.shape(id).cloned()?;
        let idx = after.min(before.lane_partitions().len().saturating_sub(1)) + 1;
        let (at, size) = dm.shape_mut(id).map(|s| s.add_partition(after))?;
        shift_lane_contents(dm, &before, at, size);
        Some(idx)
    }

    /// Take a partition out of a swimlane; the swimlane shrinks and what stood in the
    /// partitions after it moves back. What stood in the removed partition stays where it is.
    pub(super) fn remove_partition(&mut self, id: u64, idx: usize) {
        if self.diag().and_then(|d| d.shape(id)).map(|s| s.lane_partitions().len() < 2).unwrap_or(true) {
            return;
        }
        self.checkpoint();
        let Some(dm) = self.diag_mut() else { return };
        let Some(before) = dm.shape(id).cloned() else { return };
        if let Some(Some((at, size))) = dm.shape_mut(id).map(|s| s.remove_partition(idx)) {
            shift_lane_contents(dm, &before, at + size, -size);
        }
    }

    /// End the lifeline's activation at this height (for a call whose return message is left
    /// out); its next message starts a new activation.
    pub(super) fn split_activation(&mut self, lifeline: u64, y: f32) {
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(lifeline)) {
            s.splits.push(y);
            s.splits.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Take a split away again, so the activations on either side join.
    pub(super) fn join_activations(&mut self, lifeline: u64, y: f32) {
        self.checkpoint();
        if let Some(s) = self.diag_mut().and_then(|d| d.shape_mut(lifeline)) {
            s.splits.retain(|v| (v - y).abs() > 0.01);
        }
    }

    /// Attach a note: between two shapes, or from a shape (the note) to a line.
    pub(super) fn create_anchor(&mut self, a: u64, b: u64) {
        let Some(d) = self.diag() else { return };
        let (sa, sb) = (d.shape(a).is_some(), d.shape(b).is_some());
        if sa && sb {
            self.create_connector(ConnKind::Anchor, a, b, 0.0);
            return;
        }
        let (from, to) = if sa { (a, b) } else if sb { (b, a) } else { return };
        if !d.connectors.iter().any(|c| c.id == to) {
            return;
        }
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let id = p.fresh_id();
        let Some(d) = p.diagram_mut(self.active.unwrap()) else { return };
        d.connectors.push(Connector {
            id,
            kind: ConnKind::Anchor,
            from,
            to,
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
        self.clear_selection();
        self.sel_conns.push(id);
        if !self.sticky {
            self.tool = Tool::Select;
        }
    }

    pub(super) fn delete_selection(&mut self) {
        if self.sel_shapes.is_empty() && self.sel_conns.is_empty() {
            return;
        }
        self.checkpoint();
        let mut shapes = self.sel_shapes.clone();
        // a mind map topic goes with its sub-topics
        if let Some(d) = self.diag().filter(|d| d.kind == DiagramKind::MindMap) {
            shapes = crate::model::mind_subtree(d, &shapes);
        }
        let conns = self.sel_conns.clone();
        if let Some(d) = self.diag_mut() {
            d.shapes.retain(|s| !shapes.contains(&s.id));
            d.connectors.retain(|c| !conns.contains(&c.id) && !shapes.contains(&c.from) && !shapes.contains(&c.to));
            // notes attached to a removed line go with it
            let live: std::collections::HashSet<u64> = d.connectors.iter().map(|c| c.id).chain(d.shapes.iter().map(|s| s.id)).collect();
            d.connectors.retain(|c| live.contains(&c.to));
        }
        self.clear_selection();
    }

    pub(super) fn copy_selection(&mut self) -> bool {
        let Some(d) = self.diag() else { return false };
        let shapes: Vec<Shape> = d.shapes.iter().filter(|s| self.sel_shapes.contains(&s.id)).cloned().collect();
        if shapes.is_empty() {
            return false;
        }
        let ids: Vec<u64> = shapes.iter().map(|s| s.id).collect();
        let conns: Vec<Connector> = d.connectors.iter().filter(|c| ids.contains(&c.from) && ids.contains(&c.to)).cloned().collect();
        self.clipboard = Some((shapes, conns));
        true
    }

    pub(super) fn paste_items(&mut self, shapes: Vec<Shape>, conns: Vec<Connector>, offset: f32) {
        let Some(active) = self.active else { return };
        if shapes.is_empty() {
            return;
        }
        self.checkpoint();
        let p = self.project.as_mut().unwrap();
        let mut map = HashMap::new();
        let mut new_shapes = vec![];
        for mut s in shapes {
            let n = p.fresh_id();
            map.insert(s.id, n);
            s.id = n;
            s.x += offset;
            s.y += offset;
            new_shapes.push(s);
        }
        let mut new_conns = vec![];
        for mut c in conns {
            c.id = p.fresh_id();
            c.from = map[&c.from];
            c.to = map[&c.to];
            c.y += offset;
            for p in c.points.iter_mut() {
                p.0 += offset;
                p.1 += offset;
            }
            new_conns.push(c);
        }
        self.sel_shapes = new_shapes.iter().map(|s| s.id).collect();
        self.sel_conns = new_conns.iter().map(|c| c.id).collect();
        if let Some(d) = p.diagram_mut(active) {
            d.shapes.extend(new_shapes);
            d.connectors.extend(new_conns);
        }
    }

    pub(super) fn paste(&mut self) {
        if let Some((s, c)) = self.clipboard.clone() {
            self.paste_items(s, c, 20.0);
            // the next paste lands a little further down
            if let Some((s, c)) = self.clipboard.as_mut() {
                for x in s.iter_mut() {
                    x.x += 20.0;
                    x.y += 20.0;
                }
                for x in c.iter_mut() {
                    x.y += 20.0;
                    for p in x.points.iter_mut() {
                        p.0 += 20.0;
                        p.1 += 20.0;
                    }
                }
            }
        }
    }

    pub(super) fn cut(&mut self) {
        if self.copy_selection() {
            self.delete_selection();
        }
    }

    pub(super) fn duplicate(&mut self) {
        let saved = self.clipboard.clone();
        if self.copy_selection() {
            let (s, c) = self.clipboard.clone().unwrap();
            self.paste_items(s, c, 20.0);
        }
        self.clipboard = saved;
    }

    pub(super) fn select_all(&mut self) {
        if let Some(d) = self.diag() {
            let shapes = d.shapes.iter().map(|s| s.id).collect();
            let conns = d.connectors.iter().map(|c| c.id).collect();
            self.sel_shapes = shapes;
            self.sel_conns = conns;
        }
    }

    pub(super) fn select_same_type(&mut self) {
        let Some(d) = self.diag() else { return };
        let kinds: Vec<ShapeKind> = d.shapes.iter().filter(|s| self.sel_shapes.contains(&s.id)).map(|s| s.kind).collect();
        let ckinds: Vec<ConnKind> = d.connectors.iter().filter(|c| self.sel_conns.contains(&c.id)).map(|c| c.kind).collect();
        let shapes = d.shapes.iter().filter(|s| kinds.contains(&s.kind)).map(|s| s.id).collect();
        let conns = d.connectors.iter().filter(|c| ckinds.contains(&c.kind)).map(|c| c.id).collect();
        self.sel_shapes = shapes;
        self.sel_conns = conns;
    }

    pub(super) fn select_inverse(&mut self) {
        let Some(d) = self.diag() else { return };
        let shapes = d.shapes.iter().filter(|s| !self.sel_shapes.contains(&s.id)).map(|s| s.id).collect();
        let conns = d.connectors.iter().filter(|c| !self.sel_conns.contains(&c.id)).map(|c| c.id).collect();
        self.sel_shapes = shapes;
        self.sel_conns = conns;
    }

    /// 0 left, 1 right, 2 top, 3 bottom, 4 horizontal centers, 5 vertical centers
    pub(super) fn align(&mut self, how: u8) {
        if self.sel_shapes.len() < 2 {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        let Some(d) = self.diag_mut() else { return };
        let Some(first) = d.shape(ids[0]).cloned() else { return };
        for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id)) {
            match how {
                0 => s.x = first.x,
                1 => s.x = first.x + first.w - s.w,
                2 => s.y = first.y,
                3 => s.y = first.y + first.h - s.h,
                4 => s.x = first.x + first.w / 2.0 - s.w / 2.0,
                _ => s.y = first.y + first.h / 2.0 - s.h / 2.0,
            }
        }
    }

    /// 0 width, 1 height, 2 both
    pub(super) fn same_size(&mut self, how: u8) {
        if self.sel_shapes.len() < 2 {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        let Some(d) = self.diag_mut() else { return };
        let Some(first) = d.shape(ids[0]).cloned() else { return };
        for s in d.shapes.iter_mut().filter(|s| ids.contains(&s.id) && !s.kind.fixed_size()) {
            if how != 1 {
                s.w = first.w;
            }
            if how != 0 {
                s.h = first.h;
            }
        }
    }

    pub(super) fn reorder(&mut self, front: bool) {
        if self.sel_shapes.is_empty() {
            return;
        }
        self.checkpoint();
        let ids = self.sel_shapes.clone();
        if let Some(d) = self.diag_mut() {
            let (sel, rest): (Vec<Shape>, Vec<Shape>) = d.shapes.drain(..).partition(|s| ids.contains(&s.id));
            if front {
                d.shapes = rest.into_iter().chain(sel).collect();
            } else {
                d.shapes = sel.into_iter().chain(rest).collect();
            }
        }
    }

    /// Add an attribute (or operation) row with a default text and open it for editing at once.
    pub(super) fn add_line(&mut self, operations: bool) {
        let Some(&id) = self.sel_shapes.first() else { return };
        let vis = self.project.as_ref().map(|p| p.options.default_visibility).unwrap_or('-');
        let Some(s) = self.diag().and_then(|d| d.shape(id)) else { return };
        let line = if operations {
            "+operation() : void".to_string()
        } else {
            match s.kind {
                ShapeKind::Entity | ShapeKind::DbView => "column : varchar(255)".to_string(),
                ShapeKind::Enumeration => format!("LITERAL{}", s.attributes.len() + 1),
                ShapeKind::Instance => "slot = value".to_string(),
                ShapeKind::State => "do / activity".to_string(),
                _ => format!("{vis}attribute : String"),
            }
        };
        self.start_line_edit(id, operations, None, Some(line));
    }

    pub(super) fn set_arrows(&mut self, id: u64, end: Option<bool>, start: Option<bool>) {
        use crate::model::{NAV_TRUE, NAV_UNSPECIFIED};
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            c.arrow_end = false;
            c.arrow_start = false;
            let set = |me: &mut u8, other: &mut u8, on: bool| {
                if on {
                    *me = NAV_TRUE;
                    if *other == NAV_TRUE {
                        *other = NAV_UNSPECIFIED;
                    }
                } else if *me == NAV_TRUE {
                    *me = NAV_UNSPECIFIED;
                }
            };
            if let Some(e) = end {
                set(&mut c.nav_to, &mut c.nav_from, e);
            }
            if let Some(s) = start {
                set(&mut c.nav_from, &mut c.nav_to, s);
            }
        }
    }

    pub(super) fn set_conn_style(&mut self, id: u64, style: u8) {
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            c.style = style;
        }
    }

    #[allow(dead_code)]
    pub(super) fn default_style(&self) -> u8 {
        self.project.as_ref().map(|p| p.options.connector_style).unwrap_or(render::STYLE_OBLIQUE)
    }

    /// Right-angle: the Rectilinear style (code 0). Straight: no bend points, Oblique (code 1).
    pub(super) fn set_line_shape(&mut self, id: u64, right_angle: bool) {
        self.checkpoint();
        if let Some(c) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            if right_angle {
                c.style = render::STYLE_RECTILINEAR;
            } else {
                c.points.clear();
                c.style = render::STYLE_OBLIQUE;
            }
        }
    }

    #[allow(dead_code)]
    pub(super) fn set_line_elbow(&mut self, id: u64, right_angle: bool) {
        let Some(d) = self.diag() else { return };
        let Some(c) = d.connectors.iter().find(|c| c.id == id) else { return };
        let (Some(a), Some(b)) = (d.shape(c.from), d.shape(c.to)) else { return };
        let ((ax, ay), (bx, by)) = (a.center(), b.center());
        let points = if !right_angle {
            vec![]
        } else if c.from == c.to {
            let (r, t) = (a.x + a.w + 30.0, a.y - 30.0);
            vec![(ax, t), (r, t), (r, ay)]
        } else if (bx - ax).abs() >= (by - ay).abs() {
            let mx = (ax + bx) / 2.0;
            vec![(mx, ay), (mx, by)]
        } else {
            let my = (ay + by) / 2.0;
            vec![(ax, my), (bx, my)]
        };
        self.checkpoint();
        if let Some(cm) = self.diag_mut().and_then(|d| d.conn_mut(id)) {
            cm.points = points;
        }
    }
}
